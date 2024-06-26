// Copyright 2020-2023 The Jujutsu Authors
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

use clap::builder::NonEmptyStringValueParser;
use jj_lib::object_id::ObjectId as _;
use jj_lib::op_store::RefTarget;

use super::{is_fast_forward, make_branch_term};
use crate::cli_util::{CommandHelper, RevisionArg};
use crate::command_error::{user_error_with_hint, CommandError};
use crate::ui::Ui;

/// Update an existing branch to point to a certain commit
#[derive(clap::Args, Clone, Debug)]
pub struct BranchSetArgs {
    /// The branch's target revision
    #[arg(long, short)]
    revision: Option<RevisionArg>,

    /// Allow moving the branch backwards or sideways
    #[arg(long, short = 'B')]
    allow_backwards: bool,

    /// The branches to update
    #[arg(required = true, value_parser = NonEmptyStringValueParser::new())]
    names: Vec<String>,
}

pub fn cmd_branch_set(
    ui: &mut Ui,
    command: &CommandHelper,
    args: &BranchSetArgs,
) -> Result<(), CommandError> {
    let mut workspace_command = command.workspace_helper(ui)?;
    let target_commit =
        workspace_command.resolve_single_rev(args.revision.as_ref().unwrap_or(&RevisionArg::AT))?;
    let repo = workspace_command.repo().as_ref();
    let branch_names = &args.names;
    for name in branch_names {
        let old_target = repo.view().get_local_branch(name);
        if old_target.is_absent() {
            return Err(user_error_with_hint(
                format!("No such branch: {name}"),
                "Use `jj branch create` to create it.",
            ));
        }
        if !args.allow_backwards && !is_fast_forward(repo, old_target, target_commit.id()) {
            return Err(user_error_with_hint(
                format!("Refusing to move branch backwards or sideways: {name}"),
                "Use --allow-backwards to allow it.",
            ));
        }
    }

    if branch_names.len() > 1 {
        writeln!(
            ui.warning_default(),
            "Updating multiple branches: {}",
            branch_names.join(", "),
        )?;
    }

    let mut tx = workspace_command.start_transaction();
    for branch_name in branch_names {
        tx.mut_repo()
            .set_local_branch_target(branch_name, RefTarget::normal(target_commit.id().clone()));
    }
    tx.finish(
        ui,
        format!(
            "point {} to commit {}",
            make_branch_term(branch_names),
            target_commit.id().hex()
        ),
    )?;
    Ok(())
}
