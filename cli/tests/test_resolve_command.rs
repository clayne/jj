// Copyright 2022 The Jujutsu Authors
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

use std::path::Path;

use indoc::indoc;

use crate::common::create_commit_with_files;
use crate::common::CommandOutput;
use crate::common::TestEnvironment;

#[must_use]
fn get_log_output(test_env: &TestEnvironment, repo_path: &Path) -> CommandOutput {
    test_env.run_jj_in(repo_path, ["log", "-T", "bookmarks"])
}

#[test]
fn test_resolution() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file", "base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file", "a\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[("file", "b\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    // Test the setup
    insta::assert_snapshot!(get_log_output(&test_env, &repo_path), @r"
    @    conflict
    ├─╮
    │ ○  b
    ○ │  a
    ├─╯
    ○  base
    ◆
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");
    insta::assert_snapshot!(
    std::fs::read_to_string(repo_path.join("file")).unwrap()
        , @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base
    +a
    +++++++ Contents of side #2
    b
    >>>>>>> Conflict 1 of 1 ends
    ");

    let editor_script = test_env.set_up_fake_editor();
    // Check that output file starts out empty and resolve the conflict
    std::fs::write(
        &editor_script,
        ["dump editor0", "write\nresolution\n"].join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv e069f073 conflict | conflict
    Parent commit      : zsuskuln aa493daf a | a
    Parent commit      : royxmykx db6a4daf b | b
    Added 0 files, modified 1 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor0")).unwrap(), @"");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    index 0000000000..88425ec521 100644
    --- a/file
    +++ b/file
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base
    -+a
    -+++++++ Contents of side #2
    -b
    ->>>>>>> Conflict 1 of 1 ends
    +resolution
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    ------- stderr -------
    Error: No conflicts found at this revision
    [EOF]
    [exit status: 2]
    ");

    // Try again with --tool=<name>
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    std::fs::write(&editor_script, "write\nresolution\n").unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            "--config=ui.merge-editor='false'",
            "--tool=fake-editor",
        ],
    );
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv 1a70c7c6 conflict | conflict
    Parent commit      : zsuskuln aa493daf a | a
    Parent commit      : royxmykx db6a4daf b | b
    Added 0 files, modified 1 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    index 0000000000..88425ec521 100644
    --- a/file
    +++ b/file
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base
    -+a
    -+++++++ Contents of side #2
    -b
    ->>>>>>> Conflict 1 of 1 ends
    +resolution
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    ------- stderr -------
    Error: No conflicts found at this revision
    [EOF]
    [exit status: 2]
    ");

    // Check that the output file starts with conflict markers if
    // `merge-tool-edits-conflict-markers=true`
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    std::fs::write(
        &editor_script,
        ["dump editor1", "write\nresolution\n"].join("\0"),
    )
    .unwrap();
    test_env
        .run_jj_in(
            &repo_path,
            [
                "resolve",
                "--config=merge-tools.fake-editor.merge-tool-edits-conflict-markers=true",
            ],
        )
        .success();
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor1")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base
    +a
    +++++++ Contents of side #2
    b
    >>>>>>> Conflict 1 of 1 ends
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    index 0000000000..88425ec521 100644
    --- a/file
    +++ b/file
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base
    -+a
    -+++++++ Contents of side #2
    -b
    ->>>>>>> Conflict 1 of 1 ends
    +resolution
    [EOF]
    ");

    // Check that if merge tool leaves conflict markers in output file and
    // `merge-tool-edits-conflict-markers=true`, these markers are properly parsed.
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    std::fs::write(
        &editor_script,
        [
            "dump editor2",
            indoc! {"
                write
                <<<<<<<
                %%%%%%%
                -some
                +fake
                +++++++
                conflict
                >>>>>>>
            "},
        ]
        .join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            "--config=merge-tools.fake-editor.merge-tool-edits-conflict-markers=true",
        ],
    );
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv 608a2310 conflict | (conflict) conflict
    Parent commit      : zsuskuln aa493daf a | a
    Parent commit      : royxmykx db6a4daf b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv 608a2310 conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor2")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base
    +a
    +++++++ Contents of side #2
    b
    >>>>>>> Conflict 1 of 1 ends
    ");
    // Note the "Modified" below
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    --- a/file
    +++ b/file
    @@ -1,7 +1,7 @@
     <<<<<<< Conflict 1 of 1
     %%%%%%% Changes from base to side #1
    --base
    -+a
    +-some
    ++fake
     +++++++ Contents of side #2
    -b
    +conflict
     >>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");

    // Check that if merge tool leaves conflict markers in output file but
    // `merge-tool-edits-conflict-markers=false` or is not specified,
    // `jj` considers the conflict resolved.
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    std::fs::write(
        &editor_script,
        [
            "dump editor3",
            indoc! {"
                write
                <<<<<<<
                %%%%%%%
                -some
                +fake
                +++++++
                conflict
                >>>>>>>
            "},
        ]
        .join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv 3166dfd2 conflict | conflict
    Parent commit      : zsuskuln aa493daf a | a
    Parent commit      : royxmykx db6a4daf b | b
    Added 0 files, modified 1 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor3")).unwrap(), @"");
    // Note the "Resolved" below
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    index 0000000000..0610716cc1 100644
    --- a/file
    +++ b/file
    @@ -1,7 +1,7 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base
    -+a
    -+++++++ Contents of side #2
    -b
    ->>>>>>> Conflict 1 of 1 ends
    +<<<<<<<
    +%%%%%%%
    +-some
    ++fake
    ++++++++
    +conflict
    +>>>>>>>
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    ------- stderr -------
    Error: No conflicts found at this revision
    [EOF]
    [exit status: 2]
    ");

    // Check that merge tool can override conflict marker style setting, and that
    // the merge tool can output Git-style conflict markers
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    std::fs::write(
        &editor_script,
        [
            "dump editor4",
            indoc! {"
                write
                <<<<<<<
                some
                |||||||
                fake
                =======
                conflict
                >>>>>>>
            "},
        ]
        .join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            "--config=merge-tools.fake-editor.merge-tool-edits-conflict-markers=true",
            "--config=merge-tools.fake-editor.conflict-marker-style=git",
        ],
    );
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv 8e03fefa conflict | (conflict) conflict
    Parent commit      : zsuskuln aa493daf a | a
    Parent commit      : royxmykx db6a4daf b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv 8e03fefa conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor4")).unwrap(), @r"
    <<<<<<< Side #1 (Conflict 1 of 1)
    a
    ||||||| Base
    base
    =======
    b
    >>>>>>> Side #2 (Conflict 1 of 1 ends)
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    --- a/file
    +++ b/file
    @@ -1,7 +1,7 @@
     <<<<<<< Conflict 1 of 1
     %%%%%%% Changes from base to side #1
    --base
    -+a
    +-fake
    ++some
     +++++++ Contents of side #2
    -b
    +conflict
     >>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");

    // Check that merge tool can leave conflict markers by returning exit code 1
    // when using `merge-conflict-exit-codes = [1]`. The Git "diff3" conflict
    // markers should also be parsed correctly.
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    std::fs::write(
        &editor_script,
        [
            "dump editor5",
            indoc! {"
                write
                <<<<<<<
                some
                |||||||
                fake
                =======
                conflict
                >>>>>>>
            "},
            "fail",
        ]
        .join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            "--config=merge-tools.fake-editor.merge-conflict-exit-codes=[1]",
        ],
    );
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv a786ac2f conflict | (conflict) conflict
    Parent commit      : zsuskuln aa493daf a | a
    Parent commit      : royxmykx db6a4daf b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv a786ac2f conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor5")).unwrap(), @"");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    --- a/file
    +++ b/file
    @@ -1,7 +1,7 @@
     <<<<<<< Conflict 1 of 1
     %%%%%%% Changes from base to side #1
    --base
    -+a
    +-fake
    ++some
     +++++++ Contents of side #2
    -b
    +conflict
     >>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");

    // Check that an error is reported if a merge tool indicated it would leave
    // conflict markers, but the output file didn't contain valid conflict markers.
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    std::fs::write(
        &editor_script,
        [
            indoc! {"
                write
                <<<<<<< this isn't diff3 style!
                some
                =======
                conflict
                >>>>>>>
            "},
            "fail",
        ]
        .join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            "--config=merge-tools.fake-editor.merge-conflict-exit-codes=[1]",
        ],
    );
    insta::assert_snapshot!(output.normalize_stderr_exit_status(), @r"
    ------- stderr -------
    Resolving conflicts in: file
    Error: Failed to resolve conflicts
    Caused by: Tool exited with exit status: 1, but did not produce valid conflict markers (run with --debug to see the exact invocation)
    [EOF]
    [exit status: 1]
    ");

    // TODO: Check that running `jj new` and then `jj resolve -r conflict` works
    // correctly.
}

fn check_resolve_produces_input_file(
    test_env: &mut TestEnvironment,
    repo_path: &Path,
    filename: &str,
    role: &str,
    expected_content: &str,
) {
    let editor_script = test_env.set_up_fake_editor();
    std::fs::write(editor_script, format!("expect\n{expected_content}")).unwrap();

    let merge_arg_config = format!(r#"merge-tools.fake-editor.merge-args=["${role}"]"#);
    // This error means that fake-editor exited successfully but did not modify the
    // output file.
    let output = test_env.run_jj_in(
        repo_path,
        ["resolve", "--config", &merge_arg_config, filename],
    );
    insta::allow_duplicates! {
        insta::assert_snapshot!(
            output.normalize_stderr_with(|s| s.replacen(filename, "$FILENAME", 1)), @r"
        ------- stderr -------
        Resolving conflicts in: $FILENAME
        Error: Failed to resolve conflicts
        Caused by: The output file is either unchanged or empty after the editor quit (run with --debug to see the exact invocation).
        [EOF]
        [exit status: 1]
        ");
    }
}

#[test]
fn test_normal_conflict_input_files() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file", "base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file", "a\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[("file", "b\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    // Test the setup
    insta::assert_snapshot!(get_log_output(&test_env, &repo_path), @r"
    @    conflict
    ├─╮
    │ ○  b
    ○ │  a
    ├─╯
    ○  base
    ◆
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");
    insta::assert_snapshot!(
    std::fs::read_to_string(repo_path.join("file")).unwrap()
        , @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base
    +a
    +++++++ Contents of side #2
    b
    >>>>>>> Conflict 1 of 1 ends
    ");

    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "base", "base\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "left", "a\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "right", "b\n");
}

#[test]
fn test_baseless_conflict_input_files() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(&test_env.work_dir(&repo_path), "base", &[], &[]);
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file", "a\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[("file", "b\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    // Test the setup
    insta::assert_snapshot!(get_log_output(&test_env, &repo_path), @r"
    @    conflict
    ├─╮
    │ ○  b
    ○ │  a
    ├─╯
    ○  base
    ◆
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");
    insta::assert_snapshot!(
    std::fs::read_to_string(repo_path.join("file")).unwrap()
        , @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    +a
    +++++++ Contents of side #2
    b
    >>>>>>> Conflict 1 of 1 ends
    ");

    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "base", "");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "left", "a\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "right", "b\n");
}

#[test]
fn test_too_many_parents() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file", "base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file", "a\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[("file", "b\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "c",
        &["base"],
        &[("file", "c\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "conflict",
        &["a", "b", "c"],
        &[],
    );
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    3-sided conflict
    [EOF]
    ");
    // Test warning color
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list", "--color=always"]), @r"
    file    [38;5;1m3-sided[38;5;3m conflict[39m
    [EOF]
    ");

    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Hint: Using default editor ':builtin'; run `jj config set --user ui.merge-editor :builtin` to disable this message.
    Error: Failed to resolve conflicts
    Caused by: The conflict at "file" has 3 sides. At most 2 sides are supported.
    [EOF]
    [exit status: 1]
    "#);
}

#[test]
fn test_simplify_conflict_sides() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Creates a 4-sided conflict, with fileA and fileB having different conflicts:
    // fileA: A - B + C - B + B - B + B
    // fileB: A - A + A - A + B - C + D
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("fileA", "base\n"), ("fileB", "base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a1",
        &["base"],
        &[("fileA", "1\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a2",
        &["base"],
        &[("fileA", "2\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b1",
        &["base"],
        &[("fileB", "1\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b2",
        &["base"],
        &[("fileB", "2\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "conflictA",
        &["a1", "a2"],
        &[],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "conflictB",
        &["b1", "b2"],
        &[],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "conflict",
        &["conflictA", "conflictB"],
        &[],
    );

    // Even though the tree-level conflict is a 4-sided conflict, each file is
    // materialized as a 2-sided conflict.
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["debug", "tree"]), @r#"
    fileA: Ok(Conflicted([Some(File { id: FileId("d00491fd7e5bb6fa28c517a0bb32b8b506539d4d"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("0cfbf08886fca9a91cb753ec8734c84fcbe52c9f"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false })]))
    fileB: Ok(Conflicted([Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("d00491fd7e5bb6fa28c517a0bb32b8b506539d4d"), executable: false }), Some(File { id: FileId("df967b96a579e45a18b8251732d16804b2e56a55"), executable: false }), Some(File { id: FileId("0cfbf08886fca9a91cb753ec8734c84fcbe52c9f"), executable: false })]))
    [EOF]
    "#);
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    fileA    2-sided conflict
    fileB    2-sided conflict
    [EOF]
    ");
    insta::assert_snapshot!(
    std::fs::read_to_string(repo_path.join("fileA")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base
    +1
    +++++++ Contents of side #2
    2
    >>>>>>> Conflict 1 of 1 ends
    ");
    insta::assert_snapshot!(
    std::fs::read_to_string(repo_path.join("fileB")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base
    +1
    +++++++ Contents of side #2
    2
    >>>>>>> Conflict 1 of 1 ends
    ");

    // Conflict should be simplified before being handled by external merge tool.
    check_resolve_produces_input_file(&mut test_env, &repo_path, "fileA", "base", "base\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "fileA", "left", "1\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "fileA", "right", "2\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "fileB", "base", "base\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "fileB", "left", "1\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "fileB", "right", "2\n");

    // Check that simplified conflicts are still parsed as conflicts after editing
    // when `merge-tool-edits-conflict-markers=true`.
    let editor_script = test_env.set_up_fake_editor();
    std::fs::write(
        editor_script,
        indoc! {"
            write
            <<<<<<< Conflict 1 of 1
            %%%%%%% Changes from base to side #1
            -base_edited
            +1_edited
            +++++++ Contents of side #2
            2_edited
            >>>>>>> Conflict 1 of 1 ends
        "},
    )
    .unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            "--config=merge-tools.fake-editor.merge-tool-edits-conflict-markers=true",
            "fileB",
        ],
    );
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: fileB
    Working copy now at: nkmrtpmo 69cc0c2d conflict | (conflict) conflict
    Parent commit      : kmkuslsw 4601566f conflictA | (conflict) (empty) conflictA
    Parent commit      : lylxulpl 6f8d8381 conflictB | (conflict) (empty) conflictB
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    fileA    2-sided conflict
    fileB    2-sided conflict
    New conflicts appeared in 1 commits:
      nkmrtpmo 69cc0c2d conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new nkmrtpmo
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(std::fs::read_to_string(repo_path.join("fileB")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base_edited
    +1_edited
    +++++++ Contents of side #2
    2_edited
    >>>>>>> Conflict 1 of 1 ends
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    fileA    2-sided conflict
    fileB    2-sided conflict
    [EOF]
    ");
}

#[test]
fn test_edit_delete_conflict_input_files() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file", "base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file", "a\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "b", &["base"], &[]);
    std::fs::remove_file(repo_path.join("file")).unwrap();
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    // Test the setup
    insta::assert_snapshot!(get_log_output(&test_env, &repo_path), @r"
    @    conflict
    ├─╮
    │ ○  b
    ○ │  a
    ├─╯
    ○  base
    ◆
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict including 1 deletion
    [EOF]
    ");
    insta::assert_snapshot!(
    std::fs::read_to_string(repo_path.join("file")).unwrap()
        , @r"
    <<<<<<< Conflict 1 of 1
    +++++++ Contents of side #1
    a
    %%%%%%% Changes from base to side #2
    -base
    >>>>>>> Conflict 1 of 1 ends
    ");

    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "base", "base\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "left", "a\n");
    check_resolve_produces_input_file(&mut test_env, &repo_path, "file", "right", "");
}

#[test]
fn test_file_vs_dir() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file", "base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file", "a\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "b", &["base"], &[]);
    std::fs::remove_file(repo_path.join("file")).unwrap();
    std::fs::create_dir(repo_path.join("file")).unwrap();
    // Without a placeholder file, `jj` ignores an empty directory
    std::fs::write(repo_path.join("file").join("placeholder"), "").unwrap();
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    insta::assert_snapshot!(get_log_output(&test_env, &repo_path), @r"
    @    conflict
    ├─╮
    │ ○  b
    ○ │  a
    ├─╯
    ○  base
    ◆
    [EOF]
    ");

    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict including a directory
    [EOF]
    ");
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Hint: Using default editor ':builtin'; run `jj config set --user ui.merge-editor :builtin` to disable this message.
    Error: Failed to resolve conflicts
    Caused by: Only conflicts that involve normal files (not symlinks, not executable, etc.) are supported. Conflict summary for "file":
    Conflict:
      Removing file with id df967b96a579e45a18b8251732d16804b2e56a55
      Adding file with id 78981922613b2afb6025042ff6bd878ac1994e85
      Adding tree with id 133bb38fc4e4bf6b551f1f04db7e48f04cac2877

    [EOF]
    [exit status: 1]
    "#);
}

#[test]
fn test_description_with_dir_and_deletion() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file", "base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "edit",
        &["base"],
        &[("file", "b\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "dir", &["base"], &[]);
    std::fs::remove_file(repo_path.join("file")).unwrap();
    std::fs::create_dir(repo_path.join("file")).unwrap();
    // Without a placeholder file, `jj` ignores an empty directory
    std::fs::write(repo_path.join("file").join("placeholder"), "").unwrap();
    create_commit_with_files(&test_env.work_dir(&repo_path), "del", &["base"], &[]);
    std::fs::remove_file(repo_path.join("file")).unwrap();
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "conflict",
        &["edit", "dir", "del"],
        &[],
    );
    insta::assert_snapshot!(get_log_output(&test_env, &repo_path), @r"
    @      conflict
    ├─┬─╮
    │ │ ○  del
    │ ○ │  dir
    │ ├─╯
    ○ │  edit
    ├─╯
    ○  base
    ◆
    [EOF]
    ");

    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    3-sided conflict including 1 deletion and a directory
    [EOF]
    ");
    // Test warning color. The deletion is fine, so it's not highlighted
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list", "--color=always"]), @r"
    file    [38;5;1m3-sided[38;5;3m conflict including 1 deletion and [38;5;1ma directory[39m
    [EOF]
    ");
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Hint: Using default editor ':builtin'; run `jj config set --user ui.merge-editor :builtin` to disable this message.
    Error: Failed to resolve conflicts
    Caused by: Only conflicts that involve normal files (not symlinks, not executable, etc.) are supported. Conflict summary for "file":
    Conflict:
      Removing file with id df967b96a579e45a18b8251732d16804b2e56a55
      Removing file with id df967b96a579e45a18b8251732d16804b2e56a55
      Adding file with id 61780798228d17af2d34fce4cfbdf35556832472
      Adding tree with id 133bb38fc4e4bf6b551f1f04db7e48f04cac2877

    [EOF]
    [exit status: 1]
    "#);
}

#[test]
fn test_resolve_conflicts_with_executable() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create a conflict in "file1" where all 3 terms are executables, and create a
    // conflict in "file2" where one side set the executable bit.
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file1", "base1\n"), ("file2", "base2\n")],
    );
    test_env
        .run_jj_in(&repo_path, ["file", "chmod", "x", "file1"])
        .success();
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file1", "a1\n"), ("file2", "a2\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[("file1", "b1\n"), ("file2", "b2\n")],
    );
    test_env
        .run_jj_in(&repo_path, ["file", "chmod", "x", "file2"])
        .success();
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file1    2-sided conflict including an executable
    file2    2-sided conflict including an executable
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(repo_path.join("file1")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base1
    +a1
    +++++++ Contents of side #2
    b1
    >>>>>>> Conflict 1 of 1 ends
    "
    );
    insta::assert_snapshot!(
        std::fs::read_to_string(repo_path.join("file2")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base2
    +a2
    +++++++ Contents of side #2
    b2
    >>>>>>> Conflict 1 of 1 ends
    "
    );
    let editor_script = test_env.set_up_fake_editor();

    // Test resolving the conflict in "file1", which should produce an executable
    std::fs::write(&editor_script, b"write\nresolution1\n").unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve", "file1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file1
    Working copy now at: znkkpsqq eb159d56 conflict | (conflict) conflict
    Parent commit      : mzvwutvl 08932848 a | a
    Parent commit      : yqosqzyt b69b3de6 b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file2    2-sided conflict including an executable
    New conflicts appeared in 1 commits:
      znkkpsqq eb159d56 conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new znkkpsqq
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file1 b/file1
    index 0000000000..95cc18629d 100755
    --- a/file1
    +++ b/file1
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base1
    -+a1
    -+++++++ Contents of side #2
    -b1
    ->>>>>>> Conflict 1 of 1 ends
    +resolution1
    [EOF]
    ");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file2    2-sided conflict including an executable
    [EOF]
    ");

    // Test resolving the conflict in "file2", which should produce an executable
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    std::fs::write(&editor_script, b"write\nresolution2\n").unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve", "file2"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file2
    Working copy now at: znkkpsqq 4dccbb3c conflict | (conflict) conflict
    Parent commit      : mzvwutvl 08932848 a | a
    Parent commit      : yqosqzyt b69b3de6 b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file1    2-sided conflict including an executable
    New conflicts appeared in 1 commits:
      znkkpsqq 4dccbb3c conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new znkkpsqq
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file2 b/file2
    index 0000000000..775f078581 100755
    --- a/file2
    +++ b/file2
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base2
    -+a2
    -+++++++ Contents of side #2
    -b2
    ->>>>>>> Conflict 1 of 1 ends
    +resolution2
    [EOF]
    ");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file1    2-sided conflict including an executable
    [EOF]
    ");
}

#[test]
fn test_resolve_long_conflict_markers() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Makes it easier to read the diffs between conflicts
    test_env.add_config("ui.conflict-marker-style = 'snapshot'");

    // Create a conflict which requires long conflict markers to be materialized
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[("file", "======= base\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file", "<<<<<<< a\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[("file", ">>>>>>> b\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(repo_path.join("file")).unwrap(), @r"
    <<<<<<<<<<< Conflict 1 of 1
    +++++++++++ Contents of side #1
    <<<<<<< a
    ----------- Contents of base
    ======= base
    +++++++++++ Contents of side #2
    >>>>>>> b
    >>>>>>>>>>> Conflict 1 of 1 ends
    "
    );
    let editor_script = test_env.set_up_fake_editor();
    // Allow signaling that conflict markers were produced even if not editing
    // conflict markers materialized in the output file
    test_env.add_config("merge-tools.fake-editor.merge-conflict-exit-codes = [1]");

    // By default, conflict markers of length 7 or longer are parsed for
    // compatibility with Git merge tools
    std::fs::write(
        &editor_script,
        indoc! {b"
        write
        <<<<<<<
        A
        |||||||
        BASE
        =======
        B
        >>>>>>>
        \0fail
        "},
    )
    .unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv 2b985546 conflict | (conflict) conflict
    Parent commit      : zsuskuln 64177fd4 a | a
    Parent commit      : royxmykx db442c1e b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv 2b985546 conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    --- a/file
    +++ b/file
    @@ -1,8 +1,8 @@
    -<<<<<<<<<<< Conflict 1 of 1
    -+++++++++++ Contents of side #1
    -<<<<<<< a
    ------------ Contents of base
    -======= base
    -+++++++++++ Contents of side #2
    ->>>>>>> b
    ->>>>>>>>>>> Conflict 1 of 1 ends
    +<<<<<<< Conflict 1 of 1
    ++++++++ Contents of side #1
    +A
    +------- Contents of base
    +BASE
    ++++++++ Contents of side #2
    +B
    +>>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");

    // If the merge tool edits the output file with materialized markers, the
    // markers must match the length of the materialized markers to be parsed
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    std::fs::write(
        &editor_script,
        indoc! {b"
        dump editor
        \0write
        <<<<<<<<<<<
        <<<<<<< A
        |||||||||||
        ======= BASE
        ===========
        >>>>>>> B
        >>>>>>>>>>>
        \0fail
        "},
    )
    .unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            "--config=merge-tools.fake-editor.merge-tool-edits-conflict-markers=true",
        ],
    );
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv fac9406d conflict | (conflict) conflict
    Parent commit      : zsuskuln 64177fd4 a | a
    Parent commit      : royxmykx db442c1e b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv fac9406d conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor")).unwrap(), @r"
    <<<<<<<<<<< Conflict 1 of 1
    +++++++++++ Contents of side #1
    <<<<<<< a
    ----------- Contents of base
    ======= base
    +++++++++++ Contents of side #2
    >>>>>>> b
    >>>>>>>>>>> Conflict 1 of 1 ends
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    --- a/file
    +++ b/file
    @@ -1,8 +1,8 @@
     <<<<<<<<<<< Conflict 1 of 1
     +++++++++++ Contents of side #1
    -<<<<<<< a
    +<<<<<<< A
     ----------- Contents of base
    -======= base
    +======= BASE
     +++++++++++ Contents of side #2
    ->>>>>>> b
    +>>>>>>> B
     >>>>>>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");

    // If the merge tool accepts the marker length as an argument, then the conflict
    // markers should be at least as long as "$marker_length"
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    std::fs::write(
        &editor_script,
        indoc! {b"
        expect-arg 0
        11\0write
        <<<<<<<<<<<
        <<<<<<< A
        |||||||||||
        ======= BASE
        ===========
        >>>>>>> B
        >>>>>>>>>>>
        \0fail
        "},
    )
    .unwrap();
    let output = test_env.run_jj_in(
        &repo_path,
        [
            "resolve",
            r#"--config=merge-tools.fake-editor.merge-args=["$output", "$marker_length"]"#,
        ],
    );
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: file
    Working copy now at: vruxwmqv 1b29631a conflict | (conflict) conflict
    Parent commit      : zsuskuln 64177fd4 a | a
    Parent commit      : royxmykx db442c1e b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv 1b29631a conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file b/file
    --- a/file
    +++ b/file
    @@ -1,8 +1,8 @@
     <<<<<<<<<<< Conflict 1 of 1
     +++++++++++ Contents of side #1
    -<<<<<<< a
    +<<<<<<< A
     ----------- Contents of base
    -======= base
    +======= BASE
     +++++++++++ Contents of side #2
    ->>>>>>> b
    +>>>>>>> B
     >>>>>>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file    2-sided conflict
    [EOF]
    ");
}

#[test]
fn test_multiple_conflicts() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[
            (
                "this_file_has_a_very_long_name_to_test_padding",
                "first base\n",
            ),
            ("another_file", "second base\n"),
        ],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[
            (
                "this_file_has_a_very_long_name_to_test_padding",
                "first a\n",
            ),
            ("another_file", "second a\n"),
        ],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[
            (
                "this_file_has_a_very_long_name_to_test_padding",
                "first b\n",
            ),
            ("another_file", "second b\n"),
        ],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    // Test the setup
    insta::assert_snapshot!(get_log_output(&test_env, &repo_path), @r"
    @    conflict
    ├─╮
    │ ○  b
    ○ │  a
    ├─╯
    ○  base
    ◆
    [EOF]
    ");
    insta::assert_snapshot!(
    std::fs::read_to_string(
        repo_path.join("this_file_has_a_very_long_name_to_test_padding")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -first base
    +first a
    +++++++ Contents of side #2
    first b
    >>>>>>> Conflict 1 of 1 ends
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(repo_path.join("another_file")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -second base
    +second a
    +++++++ Contents of side #2
    second b
    >>>>>>> Conflict 1 of 1 ends
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    another_file                        2-sided conflict
    this_file_has_a_very_long_name_to_test_padding 2-sided conflict
    [EOF]
    ");
    // Test colors
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list", "--color=always"]), @r"
    another_file                        [38;5;3m2-sided conflict[39m
    this_file_has_a_very_long_name_to_test_padding [38;5;3m2-sided conflict[39m
    [EOF]
    ");

    let editor_script = test_env.set_up_fake_editor();

    // Check that we can manually pick which of the conflicts to resolve first
    std::fs::write(&editor_script, "expect\n\0write\nresolution another_file\n").unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve", "another_file"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Resolving conflicts in: another_file
    Working copy now at: vruxwmqv 309e981c conflict | (conflict) conflict
    Parent commit      : zsuskuln de7553ef a | a
    Parent commit      : royxmykx f68bc2f0 b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    this_file_has_a_very_long_name_to_test_padding 2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv 309e981c conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/another_file b/another_file
    index 0000000000..a9fcc7d486 100644
    --- a/another_file
    +++ b/another_file
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --second base
    -+second a
    -+++++++ Contents of side #2
    -second b
    ->>>>>>> Conflict 1 of 1 ends
    +resolution another_file
    [EOF]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    this_file_has_a_very_long_name_to_test_padding 2-sided conflict
    [EOF]
    ");

    // Repeat the above with the `--quiet` option.
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    std::fs::write(&editor_script, "expect\n\0write\nresolution another_file\n").unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve", "--quiet", "another_file"]);
    insta::assert_snapshot!(output, @"");

    // Without a path, `jj resolve` should call the merge tool multiple times
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    std::fs::write(
        &editor_script,
        [
            "expect\n",
            "write\nfirst resolution for auto-chosen file\n",
            "next invocation\n",
            "expect\n",
            "write\nsecond resolution for auto-chosen file\n",
        ]
        .join("\0"),
    )
    .unwrap();
    test_env.run_jj_in(&repo_path, ["resolve"]).success();
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/another_file b/another_file
    index 0000000000..7903e1c1c7 100644
    --- a/another_file
    +++ b/another_file
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --second base
    -+second a
    -+++++++ Contents of side #2
    -second b
    ->>>>>>> Conflict 1 of 1 ends
    +first resolution for auto-chosen file
    diff --git a/this_file_has_a_very_long_name_to_test_padding b/this_file_has_a_very_long_name_to_test_padding
    index 0000000000..f8c72adf17 100644
    --- a/this_file_has_a_very_long_name_to_test_padding
    +++ b/this_file_has_a_very_long_name_to_test_padding
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --first base
    -+first a
    -+++++++ Contents of side #2
    -first b
    ->>>>>>> Conflict 1 of 1 ends
    +second resolution for auto-chosen file
    [EOF]
    ");

    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    ------- stderr -------
    Error: No conflicts found at this revision
    [EOF]
    [exit status: 2]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve"]), @r"
    ------- stderr -------
    Error: No conflicts found at this revision
    [EOF]
    [exit status: 2]
    ");
}

#[test]
fn test_multiple_conflicts_with_error() {
    let mut test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let repo_path = test_env.env_root().join("repo");

    // Create two conflicted files, and one non-conflicted file
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "base",
        &[],
        &[
            ("file1", "base1\n"),
            ("file2", "base2\n"),
            ("file3", "base3\n"),
        ],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "a",
        &["base"],
        &[("file1", "a1\n"), ("file2", "a2\n")],
    );
    create_commit_with_files(
        &test_env.work_dir(&repo_path),
        "b",
        &["base"],
        &[("file1", "b1\n"), ("file2", "b2\n")],
    );
    create_commit_with_files(&test_env.work_dir(&repo_path), "conflict", &["a", "b"], &[]);
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file1    2-sided conflict
    file2    2-sided conflict
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(repo_path.join("file1")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base1
    +a1
    +++++++ Contents of side #2
    b1
    >>>>>>> Conflict 1 of 1 ends
    "
    );
    insta::assert_snapshot!(
        std::fs::read_to_string(repo_path.join("file2")).unwrap(), @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base to side #1
    -base2
    +a2
    +++++++ Contents of side #2
    b2
    >>>>>>> Conflict 1 of 1 ends
    "
    );
    let editor_script = test_env.set_up_fake_editor();

    // Test resolving one conflict, then exiting without resolving the second one
    std::fs::write(
        &editor_script,
        ["write\nresolution1\n", "next invocation\n"].join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output.normalize_stderr_exit_status(), @r"
    ------- stderr -------
    Resolving conflicts in: file1
    Resolving conflicts in: file2
    Working copy now at: vruxwmqv d2f3f858 conflict | (conflict) conflict
    Parent commit      : zsuskuln 9db7fdfb a | a
    Parent commit      : royxmykx d67e26e4 b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file2    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv d2f3f858 conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    Error: Stopped due to error after resolving 1 conflicts
    Caused by: The output file is either unchanged or empty after the editor quit (run with --debug to see the exact invocation).
    [EOF]
    [exit status: 1]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file1 b/file1
    index 0000000000..95cc18629d 100644
    --- a/file1
    +++ b/file1
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base1
    -+a1
    -+++++++ Contents of side #2
    -b1
    ->>>>>>> Conflict 1 of 1 ends
    +resolution1
    [EOF]
    ");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file2    2-sided conflict
    [EOF]
    ");

    // Test resolving one conflict, then failing during the second resolution
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    std::fs::write(
        &editor_script,
        ["write\nresolution1\n", "next invocation\n", "fail"].join("\0"),
    )
    .unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output.normalize_stderr_exit_status(), @r"
    ------- stderr -------
    Resolving conflicts in: file1
    Resolving conflicts in: file2
    Working copy now at: vruxwmqv 0a54e8ed conflict | (conflict) conflict
    Parent commit      : zsuskuln 9db7fdfb a | a
    Parent commit      : royxmykx d67e26e4 b | b
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file2    2-sided conflict
    New conflicts appeared in 1 commits:
      vruxwmqv 0a54e8ed conflict | (conflict) conflict
    Hint: To resolve the conflicts, start by updating to it:
      jj new vruxwmqv
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you may want to inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    Error: Stopped due to error after resolving 1 conflicts
    Caused by: Tool exited with exit status: 1 (run with --debug to see the exact invocation)
    [EOF]
    [exit status: 1]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @r"
    diff --git a/file1 b/file1
    index 0000000000..95cc18629d 100644
    --- a/file1
    +++ b/file1
    @@ -1,7 +1,1 @@
    -<<<<<<< Conflict 1 of 1
    -%%%%%%% Changes from base to side #1
    --base1
    -+a1
    -+++++++ Contents of side #2
    -b1
    ->>>>>>> Conflict 1 of 1 ends
    +resolution1
    [EOF]
    ");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file2    2-sided conflict
    [EOF]
    ");

    // Test immediately failing to resolve any conflict
    test_env.run_jj_in(&repo_path, ["undo"]).success();
    std::fs::write(&editor_script, "fail").unwrap();
    let output = test_env.run_jj_in(&repo_path, ["resolve"]);
    insta::assert_snapshot!(output.normalize_stderr_exit_status(), @r"
    ------- stderr -------
    Resolving conflicts in: file1
    Error: Failed to resolve conflicts
    Caused by: Tool exited with exit status: 1 (run with --debug to see the exact invocation)
    [EOF]
    [exit status: 1]
    ");
    insta::assert_snapshot!(test_env.run_jj_in(&repo_path, ["diff", "--git"]), @"");
    insta::assert_snapshot!(
        test_env.run_jj_in(&repo_path, ["resolve", "--list"]), @r"
    file1    2-sided conflict
    file2    2-sided conflict
    [EOF]
    ");
}
