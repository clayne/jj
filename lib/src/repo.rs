// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::backend::{BackendError, CommitId};
use crate::commit::Commit;
use crate::commit_builder::{new_change_id, signature, CommitBuilder};
use crate::dag_walk::topo_order_reverse;
use crate::index::{IndexRef, MutableIndex, ReadonlyIndex};
use crate::index_store::IndexStore;
use crate::op_heads_store::OpHeadsStore;
use crate::op_store::{BranchTarget, OpStore, OperationId, RefTarget, WorkspaceId};
use crate::operation::Operation;
use crate::rewrite::DescendantRebaser;
use crate::settings::{RepoSettings, UserSettings};
use crate::simple_op_store::SimpleOpStore;
use crate::store::Store;
use crate::transaction::Transaction;
use crate::view::{RefName, View};
use crate::{backend, op_store};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepoError {
    #[error("Object not found")]
    NotFound,
    #[error("Error: {0}")]
    Other(String),
}

impl From<BackendError> for RepoError {
    fn from(err: BackendError) -> Self {
        match err {
            BackendError::NotFound => RepoError::NotFound,
            BackendError::Other(description) => RepoError::Other(description),
        }
    }
}

pub type RepoResult<T> = Result<T, RepoError>;

// TODO: Should we implement From<&ReadonlyRepo> and From<&MutableRepo> for
// RepoRef?
#[derive(Clone, Copy)]
pub enum RepoRef<'a> {
    Readonly(&'a ReadonlyRepo),
    Mutable(&'a MutableRepo),
}

impl<'a> RepoRef<'a> {
    pub fn base_repo(&self) -> &ReadonlyRepo {
        match self {
            RepoRef::Readonly(repo) => repo,
            RepoRef::Mutable(repo) => repo.base_repo.as_ref(),
        }
    }

    pub fn store(&self) -> &Arc<Store> {
        match self {
            RepoRef::Readonly(repo) => repo.store(),
            RepoRef::Mutable(repo) => repo.store(),
        }
    }

    pub fn op_store(&self) -> &Arc<dyn OpStore> {
        match self {
            RepoRef::Readonly(repo) => repo.op_store(),
            RepoRef::Mutable(repo) => repo.op_store(),
        }
    }

    pub fn index(&self) -> IndexRef<'a> {
        match self {
            RepoRef::Readonly(repo) => IndexRef::Readonly(repo.index()),
            RepoRef::Mutable(repo) => IndexRef::Mutable(repo.index()),
        }
    }

    pub fn view(&self) -> &View {
        match self {
            RepoRef::Readonly(repo) => repo.view(),
            RepoRef::Mutable(repo) => repo.view(),
        }
    }
}

pub struct ReadonlyRepo {
    repo_path: PathBuf,
    store: Arc<Store>,
    op_store: Arc<dyn OpStore>,
    op_heads_store: Arc<OpHeadsStore>,
    operation: Operation,
    settings: RepoSettings,
    index_store: Arc<IndexStore>,
    index: Mutex<Option<Arc<ReadonlyIndex>>>,
    view: View,
}

impl Debug for ReadonlyRepo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("Repo")
            .field("repo_path", &self.repo_path)
            .field("store", &self.store)
            .finish()
    }
}

impl ReadonlyRepo {
    pub fn init_local(settings: &UserSettings, repo_path: PathBuf) -> Arc<ReadonlyRepo> {
        ReadonlyRepo::init_repo_dir(&repo_path);
        let store = Store::init_local(repo_path.join("store"));
        ReadonlyRepo::init(settings, repo_path, store)
    }

    /// Initializes a repo with a new Git backend in .jj/git/ (bare Git repo)
    pub fn init_internal_git(settings: &UserSettings, repo_path: PathBuf) -> Arc<ReadonlyRepo> {
        ReadonlyRepo::init_repo_dir(&repo_path);
        let store = Store::init_internal_git(repo_path.join("store"));
        ReadonlyRepo::init(settings, repo_path, store)
    }

    /// Initializes a repo with an existing Git backend at the specified path
    pub fn init_external_git(
        settings: &UserSettings,
        repo_path: PathBuf,
        git_repo_path: PathBuf,
    ) -> Arc<ReadonlyRepo> {
        ReadonlyRepo::init_repo_dir(&repo_path);
        let store = Store::init_external_git(repo_path.join("store"), git_repo_path);
        ReadonlyRepo::init(settings, repo_path, store)
    }

    fn init_repo_dir(repo_path: &Path) {
        fs::create_dir(repo_path.join("store")).unwrap();
        fs::create_dir(repo_path.join("op_store")).unwrap();
        fs::create_dir(repo_path.join("op_heads")).unwrap();
        fs::create_dir(repo_path.join("index")).unwrap();
    }

    fn init(
        user_settings: &UserSettings,
        repo_path: PathBuf,
        store: Arc<Store>,
    ) -> Arc<ReadonlyRepo> {
        let repo_settings = user_settings.with_repo(&repo_path).unwrap();

        let signature = signature(user_settings);
        let checkout_commit = backend::Commit {
            parents: vec![],
            predecessors: vec![],
            root_tree: store.empty_tree_id().clone(),
            change_id: new_change_id(),
            description: "".to_string(),
            author: signature.clone(),
            committer: signature,
            is_open: true,
        };
        let checkout_commit = store.write_commit(checkout_commit);
        let workspace_id = WorkspaceId::default();

        let op_store: Arc<dyn OpStore> = Arc::new(SimpleOpStore::init(repo_path.join("op_store")));

        let mut root_view = op_store::View::default();
        root_view
            .checkouts
            .insert(workspace_id, checkout_commit.id().clone());
        root_view.head_ids.insert(checkout_commit.id().clone());
        root_view
            .public_head_ids
            .insert(store.root_commit_id().clone());
        let (op_heads_store, init_op) =
            OpHeadsStore::init(repo_path.join("op_heads"), &op_store, &root_view);
        let op_heads_store = Arc::new(op_heads_store);

        let index_store = Arc::new(IndexStore::init(repo_path.join("index")));

        let view = View::new(root_view);

        Arc::new(ReadonlyRepo {
            repo_path,
            store,
            op_store,
            op_heads_store,
            operation: init_op,
            settings: repo_settings,
            index_store,
            index: Mutex::new(None),
            view,
        })
    }

    pub fn load(user_settings: &UserSettings, repo_path: PathBuf) -> Arc<ReadonlyRepo> {
        RepoLoader::init(user_settings, repo_path).load_at_head()
    }

    pub fn loader(&self) -> RepoLoader {
        RepoLoader {
            repo_path: self.repo_path.clone(),
            repo_settings: self.settings.clone(),
            store: self.store.clone(),
            op_store: self.op_store.clone(),
            op_heads_store: self.op_heads_store.clone(),
            index_store: self.index_store.clone(),
        }
    }

    pub fn as_repo_ref(&self) -> RepoRef {
        RepoRef::Readonly(self)
    }

    pub fn repo_path(&self) -> &PathBuf {
        &self.repo_path
    }

    pub fn op_id(&self) -> &OperationId {
        self.operation.id()
    }

    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn index(&self) -> &Arc<ReadonlyIndex> {
        let mut locked_index = self.index.lock().unwrap();
        if locked_index.is_none() {
            locked_index.replace(
                self.index_store
                    .get_index_at_op(&self.operation, &self.store),
            );
        }
        let index: &Arc<ReadonlyIndex> = locked_index.as_ref().unwrap();
        // Extend lifetime from that of mutex lock to that of self. Safe since we never
        // change value once it's been set (except in `reindex()` but that
        // requires a mutable reference).
        let index: &Arc<ReadonlyIndex> = unsafe { std::mem::transmute(index) };
        index
    }

    pub fn reindex(&mut self) -> &Arc<ReadonlyIndex> {
        self.index_store.reinit();
        {
            let mut locked_index = self.index.lock().unwrap();
            locked_index.take();
        }
        self.index()
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    pub fn op_store(&self) -> &Arc<dyn OpStore> {
        &self.op_store
    }

    pub fn op_heads_store(&self) -> &Arc<OpHeadsStore> {
        &self.op_heads_store
    }

    pub fn index_store(&self) -> &Arc<IndexStore> {
        &self.index_store
    }

    pub fn settings(&self) -> &RepoSettings {
        &self.settings
    }

    pub fn start_transaction(self: &Arc<ReadonlyRepo>, description: &str) -> Transaction {
        let mut_repo = MutableRepo::new(self.clone(), self.index().clone(), &self.view);
        Transaction::new(mut_repo, description)
    }

    pub fn reload(&self) -> Arc<ReadonlyRepo> {
        self.loader().load_at_head()
    }

    pub fn reload_at(&self, operation: &Operation) -> Arc<ReadonlyRepo> {
        self.loader().load_at(operation)
    }
}

pub struct RepoLoader {
    repo_path: PathBuf,
    repo_settings: RepoSettings,
    store: Arc<Store>,
    op_store: Arc<dyn OpStore>,
    op_heads_store: Arc<OpHeadsStore>,
    index_store: Arc<IndexStore>,
}

impl RepoLoader {
    pub fn init(user_settings: &UserSettings, repo_path: PathBuf) -> Self {
        let store_path = repo_path.join("store");
        if store_path.is_file() {
            // This is the old format. Let's be nice and upgrade any existing repos.
            // TODO: Delete this in early 2022 or so
            println!("The repo format has changed. Upgrading...");
            let mut buf = vec![];
            {
                let mut store_file = File::open(&store_path).unwrap();
                store_file.read_to_end(&mut buf).unwrap();
            }
            let contents = String::from_utf8(buf).unwrap();
            assert!(contents.starts_with("git: "));
            let git_backend_path_str = contents[5..].to_string();
            fs::remove_file(&store_path).unwrap();
            fs::create_dir(&store_path).unwrap();
            if repo_path.join("git").is_dir() {
                fs::rename(repo_path.join("git"), store_path.join("git")).unwrap();
            }
            fs::write(store_path.join("git_target"), &git_backend_path_str).unwrap();
            println!("Done. .jj/git is now .jj/store/git");
        }
        let store = Store::load_store(repo_path.join("store"));
        let repo_settings = user_settings.with_repo(&repo_path).unwrap();
        let op_store: Arc<dyn OpStore> = Arc::new(SimpleOpStore::load(repo_path.join("op_store")));
        let op_heads_store = Arc::new(OpHeadsStore::load(repo_path.join("op_heads")));
        let index_store = Arc::new(IndexStore::load(repo_path.join("index")));
        Self {
            repo_path,
            repo_settings,
            store,
            op_store,
            op_heads_store,
            index_store,
        }
    }

    pub fn repo_path(&self) -> &PathBuf {
        &self.repo_path
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    pub fn index_store(&self) -> &Arc<IndexStore> {
        &self.index_store
    }

    pub fn op_store(&self) -> &Arc<dyn OpStore> {
        &self.op_store
    }

    pub fn op_heads_store(&self) -> &Arc<OpHeadsStore> {
        &self.op_heads_store
    }

    pub fn load_at_head(&self) -> Arc<ReadonlyRepo> {
        let op = self.op_heads_store.get_single_op_head(self).unwrap();
        let view = View::new(op.view().take_store_view());
        self._finish_load(op, view)
    }

    pub fn load_at(&self, op: &Operation) -> Arc<ReadonlyRepo> {
        let view = View::new(op.view().take_store_view());
        self._finish_load(op.clone(), view)
    }

    pub fn create_from(
        &self,
        operation: Operation,
        view: View,
        index: Arc<ReadonlyIndex>,
    ) -> Arc<ReadonlyRepo> {
        let repo = ReadonlyRepo {
            repo_path: self.repo_path.clone(),
            store: self.store.clone(),
            op_store: self.op_store.clone(),
            op_heads_store: self.op_heads_store.clone(),
            operation,
            settings: self.repo_settings.clone(),
            index_store: self.index_store.clone(),
            index: Mutex::new(Some(index)),
            view,
        };
        Arc::new(repo)
    }

    fn _finish_load(&self, operation: Operation, view: View) -> Arc<ReadonlyRepo> {
        let repo = ReadonlyRepo {
            repo_path: self.repo_path.clone(),
            store: self.store.clone(),
            op_store: self.op_store.clone(),
            op_heads_store: self.op_heads_store.clone(),
            operation,
            settings: self.repo_settings.clone(),
            index_store: self.index_store.clone(),
            index: Mutex::new(None),
            view,
        };
        Arc::new(repo)
    }
}

pub struct MutableRepo {
    base_repo: Arc<ReadonlyRepo>,
    index: MutableIndex,
    view: RefCell<View>,
    view_dirty: bool,
    rewritten_commits: HashMap<CommitId, HashSet<CommitId>>,
    abandoned_commits: HashSet<CommitId>,
}

impl MutableRepo {
    pub fn new(
        base_repo: Arc<ReadonlyRepo>,
        index: Arc<ReadonlyIndex>,
        view: &View,
    ) -> MutableRepo {
        let mut_view = view.clone();
        let mut_index = MutableIndex::incremental(index);
        MutableRepo {
            base_repo,
            index: mut_index,
            view: RefCell::new(mut_view),
            view_dirty: false,
            rewritten_commits: Default::default(),
            abandoned_commits: Default::default(),
        }
    }

    pub fn as_repo_ref(&self) -> RepoRef {
        RepoRef::Mutable(self)
    }

    pub fn base_repo(&self) -> &Arc<ReadonlyRepo> {
        &self.base_repo
    }

    pub fn store(&self) -> &Arc<Store> {
        self.base_repo.store()
    }

    pub fn op_store(&self) -> &Arc<dyn OpStore> {
        self.base_repo.op_store()
    }

    pub fn index(&self) -> &MutableIndex {
        &self.index
    }

    pub fn view(&self) -> &View {
        self.enforce_view_invariants();
        let view_borrow = self.view.borrow();
        let view = view_borrow.deref();
        unsafe { std::mem::transmute(view) }
    }

    fn view_mut(&mut self) -> &mut View {
        self.view.get_mut()
    }

    pub fn has_changes(&self) -> bool {
        self.enforce_view_invariants();
        self.view.borrow().deref() != &self.base_repo.view
    }

    pub fn consume(self) -> (MutableIndex, View) {
        self.enforce_view_invariants();
        (self.index, self.view.into_inner())
    }

    pub fn write_commit(&mut self, commit: backend::Commit) -> Commit {
        let commit = self.store().write_commit(commit);
        self.add_head(&commit);
        commit
    }

    /// Record a commit as having been rewritten in this transaction. This
    /// record is used by `rebase_descendants()`.
    ///
    /// Rewritten commits don't have to be recorded here. This is just a
    /// convenient place to record it. It won't matter after the transaction
    /// has been committed.
    pub fn record_rewritten_commit(&mut self, old_id: CommitId, new_id: CommitId) {
        self.rewritten_commits
            .entry(old_id)
            .or_default()
            .insert(new_id);
    }

    pub fn clear_rewritten_commits(&mut self) {
        self.rewritten_commits.clear();
    }

    /// Record a commit as having been abandoned in this transaction. This
    /// record is used by `rebase_descendants()`.
    ///
    /// Abandoned commits don't have to be recorded here. This is just a
    /// convenient place to record it. It won't matter after the transaction
    /// has been committed.
    pub fn record_abandoned_commit(&mut self, old_id: CommitId) {
        self.abandoned_commits.insert(old_id);
    }

    pub fn clear_abandoned_commits(&mut self) {
        self.abandoned_commits.clear();
    }

    /// Creates a `DescendantRebaser` to rebase descendants of the recorded
    /// rewritten and abandoned commits.
    pub fn create_descendant_rebaser<'settings, 'repo>(
        &'repo mut self,
        settings: &'settings UserSettings,
    ) -> DescendantRebaser<'settings, 'repo> {
        DescendantRebaser::new(
            settings,
            self,
            self.rewritten_commits.clone(),
            self.abandoned_commits.clone(),
        )
    }

    pub fn rebase_descendants(&mut self, settings: &UserSettings) -> usize {
        let mut rebaser = self.create_descendant_rebaser(settings);
        rebaser.rebase_all();
        rebaser.rebased().len()
    }

    pub fn get_checkout(&mut self) -> CommitId {
        self.view.borrow().checkout().clone()
    }

    pub fn set_checkout(&mut self, id: CommitId) {
        self.view_mut().set_checkout(id);
    }

    pub fn check_out(&mut self, settings: &UserSettings, commit: &Commit) -> Commit {
        let current_checkout_id = self.view.borrow().checkout().clone();
        let current_checkout = self.store().get_commit(&current_checkout_id).unwrap();
        assert!(current_checkout.is_open(), "current checkout is closed");
        if current_checkout.is_empty() {
            // Abandon the checkout we're leaving if it's empty.
            self.record_abandoned_commit(current_checkout_id);
        }
        let open_commit = if !commit.is_open() {
            // If the commit is closed, create a new open commit on top
            CommitBuilder::for_open_commit(
                settings,
                self.store(),
                commit.id().clone(),
                commit.tree().id().clone(),
            )
            .write_to_repo(self)
        } else {
            // Otherwise the commit was open, so just use that commit as is.
            commit.clone()
        };
        let id = open_commit.id().clone();
        self.set_checkout(id);
        open_commit
    }

    fn enforce_view_invariants(&self) {
        if !self.view_dirty {
            return;
        }
        let mut view_borrow_mut = self.view.borrow_mut();
        let view = view_borrow_mut.store_view_mut();
        view.public_head_ids = self
            .index
            .heads(view.public_head_ids.iter())
            .iter()
            .cloned()
            .collect();
        view.head_ids.extend(view.public_head_ids.iter().cloned());
        view.head_ids = self
            .index
            .heads(view.head_ids.iter())
            .iter()
            .cloned()
            .collect();
    }

    pub fn add_head(&mut self, head: &Commit) {
        let current_heads = self.view.get_mut().heads();
        // Use incremental update for common case of adding a single commit on top a
        // current head. TODO: Also use incremental update when adding a single
        // commit on top a non-head.
        if head
            .parent_ids()
            .iter()
            .all(|parent_id| current_heads.contains(parent_id))
        {
            self.index.add_commit(head);
            self.view.get_mut().add_head(head.id());
            for parent_id in head.parent_ids() {
                self.view.get_mut().remove_head(&parent_id);
            }
        } else {
            let missing_commits = topo_order_reverse(
                vec![head.clone()],
                Box::new(|commit: &Commit| commit.id().clone()),
                Box::new(|commit: &Commit| -> Vec<Commit> {
                    commit
                        .parents()
                        .into_iter()
                        .filter(|parent| !self.index.has_id(parent.id()))
                        .collect()
                }),
            );
            for missing_commit in missing_commits.iter().rev() {
                self.index.add_commit(missing_commit);
            }
            self.view.get_mut().add_head(head.id());
            self.view_dirty = true;
        }
    }

    pub fn remove_head(&mut self, head: &CommitId) {
        self.view_mut().remove_head(head);
        self.view_dirty = true;
    }

    pub fn add_public_head(&mut self, head: &Commit) {
        self.view_mut().add_public_head(head.id());
        self.view_dirty = true;
    }

    pub fn remove_public_head(&mut self, head: &CommitId) {
        self.view_mut().remove_public_head(head);
        self.view_dirty = true;
    }

    pub fn get_branch(&self, name: &str) -> Option<BranchTarget> {
        self.view.borrow().get_branch(name).cloned()
    }

    pub fn set_branch(&mut self, name: String, target: BranchTarget) {
        self.view_mut().set_branch(name, target);
    }

    pub fn remove_branch(&mut self, name: &str) {
        self.view_mut().remove_branch(name);
    }

    pub fn get_local_branch(&self, name: &str) -> Option<RefTarget> {
        self.view.borrow().get_local_branch(name)
    }

    pub fn set_local_branch(&mut self, name: String, target: RefTarget) {
        self.view_mut().set_local_branch(name, target);
    }

    pub fn remove_local_branch(&mut self, name: &str) {
        self.view_mut().remove_local_branch(name);
    }

    pub fn get_remote_branch(&self, name: &str, remote_name: &str) -> Option<RefTarget> {
        self.view.borrow().get_remote_branch(name, remote_name)
    }

    pub fn set_remote_branch(&mut self, name: String, remote_name: String, target: RefTarget) {
        self.view_mut().set_remote_branch(name, remote_name, target);
    }

    pub fn remove_remote_branch(&mut self, name: &str, remote_name: &str) {
        self.view_mut().remove_remote_branch(name, remote_name);
    }

    pub fn get_tag(&self, name: &str) -> Option<RefTarget> {
        self.view.borrow().get_tag(name)
    }

    pub fn set_tag(&mut self, name: String, target: RefTarget) {
        self.view_mut().set_tag(name, target);
    }

    pub fn remove_tag(&mut self, name: &str) {
        self.view_mut().remove_tag(name);
    }

    pub fn set_git_ref(&mut self, name: String, target: RefTarget) {
        self.view_mut().set_git_ref(name, target);
    }

    pub fn remove_git_ref(&mut self, name: &str) {
        self.view_mut().remove_git_ref(name);
    }

    pub fn set_git_head(&mut self, head_id: CommitId) {
        self.view_mut().set_git_head(head_id);
    }

    pub fn clear_git_head(&mut self) {
        self.view_mut().clear_git_head();
    }

    pub fn set_view(&mut self, data: op_store::View) {
        self.view_mut().set_view(data);
        self.view_dirty = true;
    }

    pub fn merge(&mut self, base_repo: &ReadonlyRepo, other_repo: &ReadonlyRepo) {
        // First, merge the index, so we can take advantage of a valid index when
        // merging the view. Merging in base_repo's index isn't typically
        // necessary, but it can be if base_repo is ahead of either self or other_repo
        // (e.g. because we're undoing an operation that hasn't been published).
        self.index.merge_in(base_repo.index());
        self.index.merge_in(other_repo.index());

        self.enforce_view_invariants();
        self.view
            .get_mut()
            .merge(self.index.as_index_ref(), &base_repo.view, &other_repo.view);
        self.view_dirty = true;
    }

    pub fn merge_single_ref(
        &mut self,
        ref_name: &RefName,
        base_target: Option<&RefTarget>,
        other_target: Option<&RefTarget>,
    ) {
        self.view.get_mut().merge_single_ref(
            self.index.as_index_ref(),
            ref_name,
            base_target,
            other_target,
        );
    }
}
