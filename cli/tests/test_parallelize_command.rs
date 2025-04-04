// Copyright 2024 The Jujutsu Authors
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

use crate::common::CommandOutput;
use crate::common::TestEnvironment;
use crate::common::TestWorkDir;

#[test]
fn test_parallelize_no_descendants() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    for n in 1..6 {
        work_dir.run_jj(["commit", &format!("-m{n}")]).success();
    }
    work_dir.run_jj(["describe", "-m=6"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  02b7709cc4e9 6 parents: 5
    ○  1b2f08d76b66 5 parents: 4
    ○  e5c4cf44e237 4 parents: 3
    ○  4cd999dfaac0 3 parents: 2
    ○  d3902619fade 2 parents: 1
    ○  8b64ddff700d 1 parents:
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(1)::"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  4850b4629edb 6 parents:
    │ ○  87627fbb7d29 5 parents:
    ├─╯
    │ ○  5b9815e28fae 4 parents:
    ├─╯
    │ ○  bb1bb465ccc2 3 parents:
    ├─╯
    │ ○  337eca1ef3a8 2 parents:
    ├─╯
    │ ○  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

// Only the head commit has descendants.
#[test]
fn test_parallelize_with_descendants_simple() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    for n in 1..6 {
        work_dir.run_jj(["commit", &format!("-m{n}")]).success();
    }
    work_dir.run_jj(["describe", "-m=6"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  02b7709cc4e9 6 parents: 5
    ○  1b2f08d76b66 5 parents: 4
    ○  e5c4cf44e237 4 parents: 3
    ○  4cd999dfaac0 3 parents: 2
    ○  d3902619fade 2 parents: 1
    ○  8b64ddff700d 1 parents:
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(1)::description(4)"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  9bc057f8b6e3 6 parents: 5
    ○        9e36a8afe793 5 parents: 1 2 3 4
    ├─┬─┬─╮
    │ │ │ ○  5b9815e28fae 4 parents:
    │ │ ○ │  bb1bb465ccc2 3 parents:
    │ │ ├─╯
    │ ○ │  337eca1ef3a8 2 parents:
    │ ├─╯
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

// One of the commits being parallelized has a child that isn't being
// parallelized. That child will become a merge of any ancestors which are being
// parallelized.
#[test]
fn test_parallelize_where_interior_has_non_target_children() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    for n in 1..6 {
        work_dir.run_jj(["commit", &format!("-m{n}")]).success();
    }
    work_dir
        .run_jj(["new", "description(2)", "-m=2c"])
        .success();
    work_dir.run_jj(["new", "description(5)", "-m=6"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  2508ea92308a 6 parents: 5
    ○  1b2f08d76b66 5 parents: 4
    ○  e5c4cf44e237 4 parents: 3
    ○  4cd999dfaac0 3 parents: 2
    │ ○  3e7571e62c87 2c parents: 2
    ├─╯
    ○  d3902619fade 2 parents: 1
    ○  8b64ddff700d 1 parents:
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(1)::description(4)"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  c9525dff9d03 6 parents: 5
    ○        b3ad09518546 5 parents: 1 2 3 4
    ├─┬─┬─╮
    │ │ │ ○  3b125ed6a683 4 parents:
    │ │ ○ │  1ed8c0c5be30 3 parents:
    │ │ ├─╯
    │ │ │ ○  c01d8e85ea96 2c parents: 1 2
    ╭─┬───╯
    │ ○ │  7efea6c89b60 2 parents:
    │ ├─╯
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_where_root_has_non_target_children() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    for n in 1..4 {
        work_dir.run_jj(["commit", &format!("-m{n}")]).success();
    }
    work_dir
        .run_jj(["new", "description(1)", "-m=1c"])
        .success();
    work_dir.run_jj(["new", "description(3)", "-m=4"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  9132691e6256 4 parents: 3
    ○  4cd999dfaac0 3 parents: 2
    ○  d3902619fade 2 parents: 1
    │ ○  6c64110df0a5 1c parents: 1
    ├─╯
    ○  8b64ddff700d 1 parents:
    ◆  000000000000 parents:
    [EOF]
    ");
    work_dir
        .run_jj(["parallelize", "description(1)::description(3)"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @      3397916989e7 4 parents: 1 2 3
    ├─┬─╮
    │ │ ○  1f768c1bc591 3 parents:
    │ ○ │  12ef12b4640e 2 parents:
    │ ├─╯
    │ │ ○  6c64110df0a5 1c parents: 1
    ├───╯
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

// One of the commits being parallelized has a child that is a merge commit.
#[test]
fn test_parallelize_with_merge_commit_child() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.run_jj(["commit", "-m", "1"]).success();
    for n in 2..4 {
        work_dir.run_jj(["commit", "-m", &n.to_string()]).success();
    }
    work_dir.run_jj(["new", "root()", "-m", "a"]).success();
    work_dir
        .run_jj(["new", "description(2)", "description(a)", "-m", "2a-c"])
        .success();
    work_dir
        .run_jj(["new", "description(3)", "-m", "4"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  99ffaf5b3984 4 parents: 3
    ○  4cd999dfaac0 3 parents: 2
    │ ○  4313cc3b476f 2a-c parents: 2 a
    ╭─┤
    │ ○  1eb902150bb9 a parents:
    ○ │  d3902619fade 2 parents: 1
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");

    // After this finishes, child-2a will have three parents: "1", "2", and "a".
    work_dir
        .run_jj(["parallelize", "description(1)::description(3)"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @      3ee9279847a6 4 parents: 1 2 3
    ├─┬─╮
    │ │ ○  bb1bb465ccc2 3 parents:
    │ │ │ ○  c70ee196514b 2a-c parents: 1 2 a
    ╭─┬───┤
    │ │ │ ○  1eb902150bb9 a parents:
    │ │ ├─╯
    │ ○ │  337eca1ef3a8 2 parents:
    │ ├─╯
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_disconnected_target_commits() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    for n in 1..3 {
        work_dir.run_jj(["commit", &format!("-m{n}")]).success();
    }
    work_dir.run_jj(["describe", "-m=3"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  4cd999dfaac0 3 parents: 2
    ○  d3902619fade 2 parents: 1
    ○  8b64ddff700d 1 parents:
    ◆  000000000000 parents:
    [EOF]
    ");

    let output = work_dir.run_jj(["parallelize", "description(1)", "description(3)"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  4cd999dfaac0 3 parents: 2
    ○  d3902619fade 2 parents: 1
    ○  8b64ddff700d 1 parents:
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_head_is_a_merge() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["commit", "-m=0"]).success();
    work_dir.run_jj(["commit", "-m=1"]).success();
    work_dir.run_jj(["commit", "-m=2"]).success();
    work_dir.run_jj(["new", "root()"]).success();
    work_dir.run_jj(["commit", "-m=a"]).success();
    work_dir.run_jj(["commit", "-m=b"]).success();
    work_dir
        .run_jj(["new", "description(2)", "description(b)", "-m=merged-head"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    1fb53c45237e merged-head parents: 2 b
    ├─╮
    │ ○  a7bf5001cfd8 b parents: a
    │ ○  6ca0450a05f5 a parents:
    ○ │  1f81bd465ed0 2 parents: 1
    ○ │  0c058af014a6 1 parents: 0
    ○ │  745bea8029c1 0 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(1)::"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    82131a679769 merged-head parents: 0 b
    ├─╮
    │ ○  a7bf5001cfd8 b parents: a
    │ ○  6ca0450a05f5 a parents:
    │ │ ○  daef04bc3fae 2 parents: 0
    ├───╯
    │ │ ○  0c058af014a6 1 parents: 0
    ├───╯
    ○ │  745bea8029c1 0 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_interior_target_is_a_merge() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["commit", "-m=0"]).success();
    work_dir.run_jj(["describe", "-m=1"]).success();
    work_dir.run_jj(["new", "root()", "-m=a"]).success();
    work_dir
        .run_jj(["new", "description(1)", "description(a)", "-m=2"])
        .success();
    work_dir.run_jj(["new", "-m=3"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  9b77792c77ac 3 parents: 2
    ○    1e29145c95fd 2 parents: 1 a
    ├─╮
    │ ○  427890ea3f2b a parents:
    ○ │  0c058af014a6 1 parents: 0
    ○ │  745bea8029c1 0 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(1)::"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    042fc3f4315c 3 parents: 0 a
    ├─╮
    │ │ ○  80603361bb48 2 parents: 0 a
    ╭─┬─╯
    │ ○  427890ea3f2b a parents:
    │ │ ○  0c058af014a6 1 parents: 0
    ├───╯
    ○ │  745bea8029c1 0 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_root_is_a_merge() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["describe", "-m=y"]).success();
    work_dir.run_jj(["new", "root()", "-m=x"]).success();
    work_dir
        .run_jj(["new", "description(y)", "description(x)", "-m=1"])
        .success();
    work_dir.run_jj(["new", "-m=2"]).success();
    work_dir.run_jj(["new", "-m=3"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  cc239b744d01 3 parents: 2
    ○  2bf00c2ad44c 2 parents: 1
    ○    1c6853121f3c 1 parents: y x
    ├─╮
    │ ○  4035b23c8f72 x parents:
    ○ │  ca57511e158f y parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(1)::description(2)"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    2c7fdfa00b38 3 parents: 1 2
    ├─╮
    │ ○    3acbd32944d6 2 parents: y x
    │ ├─╮
    ○ │ │  1c6853121f3c 1 parents: y x
    ╰─┬─╮
      │ ○  4035b23c8f72 x parents:
      ○ │  ca57511e158f y parents:
      ├─╯
      ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_multiple_heads() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["commit", "-m=0"]).success();
    work_dir.run_jj(["describe", "-m=1"]).success();
    work_dir.run_jj(["new", "description(0)", "-m=2"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  97d7522f40e8 2 parents: 0
    │ ○  0c058af014a6 1 parents: 0
    ├─╯
    ○  745bea8029c1 0 parents:
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(0)::"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  e84481c26195 2 parents:
    │ ○  6270540ee067 1 parents:
    ├─╯
    │ ○  745bea8029c1 0 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

// All heads must have the same children as the other heads, but only if they
// have children. In this test only one head has children, so the command
// succeeds.
#[test]
fn test_parallelize_multiple_heads_with_and_without_children() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["commit", "-m=0"]).success();
    work_dir.run_jj(["describe", "-m=1"]).success();
    work_dir.run_jj(["new", "description(0)", "-m=2"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  97d7522f40e8 2 parents: 0
    │ ○  0c058af014a6 1 parents: 0
    ├─╯
    ○  745bea8029c1 0 parents:
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(0)", "description(1)"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  97d7522f40e8 2 parents: 0
    ○  745bea8029c1 0 parents:
    │ ○  6270540ee067 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_multiple_roots() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["describe", "-m=1"]).success();
    work_dir.run_jj(["new", "root()", "-m=a"]).success();
    work_dir
        .run_jj(["new", "description(1)", "description(a)", "-m=2"])
        .success();
    work_dir.run_jj(["new", "-m=3"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  34da938ad94a 3 parents: 2
    ○    85d5043b881d 2 parents: 1 a
    ├─╮
    │ ○  6d37472c632c a parents:
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");

    // Succeeds because the roots have the same parents.
    work_dir.run_jj(["parallelize", "root().."]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  3c90598481cd 3 parents:
    │ ○  b96aa55582e5 2 parents:
    ├─╯
    │ ○  6d37472c632c a parents:
    ├─╯
    │ ○  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_multiple_heads_with_different_children() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["commit", "-m=1"]).success();
    work_dir.run_jj(["commit", "-m=2"]).success();
    work_dir.run_jj(["commit", "-m=3"]).success();
    work_dir.run_jj(["new", "root()"]).success();
    work_dir.run_jj(["commit", "-m=a"]).success();
    work_dir.run_jj(["commit", "-m=b"]).success();
    work_dir.run_jj(["commit", "-m=c"]).success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  4bc4dace0e65 parents: c
    ○  63b0da9212c0 c parents: b
    ○  a7bf5001cfd8 b parents: a
    ○  6ca0450a05f5 a parents:
    │ ○  4cd999dfaac0 3 parents: 2
    │ ○  d3902619fade 2 parents: 1
    │ ○  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj([
            "parallelize",
            "description(1)::description(2)",
            "description(a)::description(b)",
        ])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  f6c9d9ee3db8 parents: c
    ○    62661d5f0c77 c parents: a b
    ├─╮
    │ ○  c9ea9058f5c7 b parents:
    ○ │  6ca0450a05f5 a parents:
    ├─╯
    │ ○    dac1be696563 3 parents: 1 2
    │ ├─╮
    │ │ ○  7efea6c89b60 2 parents:
    ├───╯
    │ ○  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_multiple_roots_with_different_parents() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["commit", "-m=1"]).success();
    work_dir.run_jj(["commit", "-m=2"]).success();
    work_dir.run_jj(["new", "root()"]).success();
    work_dir.run_jj(["commit", "-m=a"]).success();
    work_dir.run_jj(["commit", "-m=b"]).success();
    work_dir
        .run_jj(["new", "description(2)", "description(b)", "-m=merged-head"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    ba4297d53c1a merged-head parents: 2 b
    ├─╮
    │ ○  6577defaca2d b parents: a
    │ ○  1eb902150bb9 a parents:
    ○ │  d3902619fade 2 parents: 1
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");

    work_dir
        .run_jj(["parallelize", "description(2)::", "description(b)::"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    0943ed52b3ed merged-head parents: 1 a
    ├─╮
    │ │ ○  6577defaca2d b parents: a
    │ ├─╯
    │ ○  1eb902150bb9 a parents:
    │ │ ○  d3902619fade 2 parents: 1
    ├───╯
    ○ │  8b64ddff700d 1 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[test]
fn test_parallelize_complex_nonlinear_target() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["new", "-m=0", "root()"]).success();
    work_dir.run_jj(["new", "-m=1", "description(0)"]).success();
    work_dir.run_jj(["new", "-m=2", "description(0)"]).success();
    work_dir.run_jj(["new", "-m=3", "description(0)"]).success();
    work_dir.run_jj(["new", "-m=4", "all:heads(..)"]).success();
    work_dir
        .run_jj(["new", "-m=1c", "description(1)"])
        .success();
    work_dir
        .run_jj(["new", "-m=2c", "description(2)"])
        .success();
    work_dir
        .run_jj(["new", "-m=3c", "description(3)"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  b043eb81416c 3c parents: 3
    │ ○    48277ee9afe0 4 parents: 3 2 1
    ╭─┼─╮
    ○ │ │  944922f0c69f 3 parents: 0
    │ │ │ ○  9d28e8e38435 2c parents: 2
    │ ├───╯
    │ ○ │  97d7522f40e8 2 parents: 0
    ├─╯ │
    │ ○ │  6c82c22a5e35 1c parents: 1
    │ ├─╯
    │ ○  0c058af014a6 1 parents: 0
    ├─╯
    ○  745bea8029c1 0 parents:
    ◆  000000000000 parents:
    [EOF]
    ");

    let output = work_dir.run_jj(["parallelize", "description(0)::description(4)"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: yostqsxw 59a216e5 (empty) 3c
    Parent commit (@-)      : rlvkpnrz 745bea80 (empty) 0
    Parent commit (@-)      : mzvwutvl cb944786 (empty) 3
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    59a216e537c4 3c parents: 0 3
    ├─╮
    │ ○  cb9447869bf0 3 parents:
    │ │ ○  248ce1ffd76b 2c parents: 0 2
    ╭───┤
    │ │ ○  8f4b8ef68676 2 parents:
    │ ├─╯
    │ │ ○  55c626d090e2 1c parents: 0 1
    ╭───┤
    │ │ ○  82918d78c984 1 parents:
    │ ├─╯
    ○ │  745bea8029c1 0 parents:
    ├─╯
    │ ○  14ca4df576b3 4 parents:
    ├─╯
    ◆  000000000000 parents:
    [EOF]
    ");
}

#[must_use]
fn get_log_output(work_dir: &TestWorkDir) -> CommandOutput {
    let template = r#"
    separate(" ",
        commit_id.short(),
        description.first_line(),
        "parents:",
        parents.map(|c|c.description().first_line())
    )"#;
    work_dir.run_jj(["log", "-T", template])
}
