// Copyright 2020 The Jujutsu Authors
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

use std::io::Write as _;

use testutils::git;

use crate::common::TestEnvironment;

#[test]
fn test_gitignores() {
    let test_env = TestEnvironment::default();
    let workspace_root = test_env.env_root().join("repo");
    git::init(&workspace_root);
    test_env
        .run_jj_in(&workspace_root, ["git", "init", "--git-repo", "."])
        .success();

    // Say in core.excludesFiles that we don't want file1, file2, or file3
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(workspace_root.join(".git").join("config"))
        .unwrap();
    // Put the file in "~/my-ignores" so we also test that "~" expands to "$HOME"
    file.write_all(b"[core]\nexcludesFile=~/my-ignores\n")
        .unwrap();
    drop(file);
    std::fs::write(
        test_env.home_dir().join("my-ignores"),
        "file1\nfile2\nfile3",
    )
    .unwrap();

    // Say in .git/info/exclude that we actually do want file2 and file3
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(workspace_root.join(".git").join("info").join("exclude"))
        .unwrap();
    file.write_all(b"!file2\n!file3").unwrap();
    drop(file);

    // Say in .gitignore (in the working copy) that we actually do not want file2
    // (again)
    std::fs::write(workspace_root.join(".gitignore"), "file2").unwrap();

    // Writes some files to the working copy
    std::fs::write(workspace_root.join("file0"), "contents").unwrap();
    std::fs::write(workspace_root.join("file1"), "contents").unwrap();
    std::fs::write(workspace_root.join("file2"), "contents").unwrap();
    std::fs::write(workspace_root.join("file3"), "contents").unwrap();

    let output = test_env.run_jj_in(&workspace_root, ["diff", "-s"]);
    insta::assert_snapshot!(output, @r"
    A .gitignore
    A file0
    A file3
    [EOF]
    ");
}

#[test]
fn test_gitignores_relative_excludes_file_path() {
    let test_env = TestEnvironment::default();
    let workspace_root = test_env.env_root().join("repo");
    test_env
        .run_jj_in(".", ["git", "init", "--colocate", "repo"])
        .success();

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(workspace_root.join(".git").join("config"))
        .unwrap();
    file.write_all(b"[core]\nexcludesFile=../my-ignores\n")
        .unwrap();
    drop(file);
    std::fs::write(test_env.env_root().join("my-ignores"), "ignored\n").unwrap();

    std::fs::write(workspace_root.join("ignored"), "").unwrap();
    std::fs::write(workspace_root.join("not-ignored"), "").unwrap();

    // core.excludesFile should be resolved relative to the workspace root, not
    // to the cwd.
    std::fs::create_dir(workspace_root.join("sub")).unwrap();
    let output = test_env.run_jj_in(&workspace_root.join("sub"), ["diff", "-s"]);
    insta::assert_snapshot!(output.normalize_backslash(), @r"
    A ../not-ignored
    [EOF]
    ");
    let output = test_env.run_jj_in(".", ["-Rrepo", "diff", "-s"]);
    insta::assert_snapshot!(output.normalize_backslash(), @r"
    A repo/not-ignored
    [EOF]
    ");
}

#[test]
fn test_gitignores_ignored_file_in_target_commit() {
    let test_env = TestEnvironment::default();
    let workspace_root = test_env.env_root().join("repo");
    git::init(&workspace_root);
    test_env
        .run_jj_in(&workspace_root, ["git", "init", "--git-repo", "."])
        .success();

    // Create a commit with file "ignored" in it
    std::fs::write(workspace_root.join("ignored"), "committed contents\n").unwrap();
    test_env
        .run_jj_in(&workspace_root, ["bookmark", "create", "-r@", "with-file"])
        .success();
    let target_commit_id = test_env
        .run_jj_in(
            &workspace_root,
            ["log", "--no-graph", "-T=commit_id", "-r=@"],
        )
        .success()
        .stdout
        .into_raw();

    // Create another commit where we ignore that path
    test_env
        .run_jj_in(&workspace_root, ["new", "root()"])
        .success();
    std::fs::write(workspace_root.join("ignored"), "contents in working copy\n").unwrap();
    std::fs::write(workspace_root.join(".gitignore"), ".gitignore\nignored\n").unwrap();

    // Update to the commit with the "ignored" file
    let output = test_env.run_jj_in(&workspace_root, ["edit", "with-file"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy now at: qpvuntsm 5ada929e with-file | (no description set)
    Parent commit      : zzzzzzzz 00000000 (empty) (no description set)
    Added 1 files, modified 0 files, removed 0 files
    Warning: 1 of those updates were skipped because there were conflicting changes in the working copy.
    Hint: Inspect the changes compared to the intended target with `jj diff --from 5ada929e5d2e`.
    Discard the conflicting changes with `jj restore --from 5ada929e5d2e`.
    [EOF]
    ");
    let output = test_env.run_jj_in(
        &workspace_root,
        ["diff", "--git", "--from", &target_commit_id],
    );
    insta::assert_snapshot!(output, @r"
    diff --git a/ignored b/ignored
    index 8a69467466..4d9be5127b 100644
    --- a/ignored
    +++ b/ignored
    @@ -1,1 +1,1 @@
    -committed contents
    +contents in working copy
    [EOF]
    ");
}
