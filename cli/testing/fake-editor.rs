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

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

use clap::Parser;
use itertools::Itertools as _;

/// A fake editor, useful for testing
// It's overkill to use clap for a single argument, but we already use it in many other places...
#[derive(Parser, Debug)]
#[clap()]
struct Args {
    /// Path to the file to edit
    file: PathBuf,
    /// Other arguments to the editor
    other_args: Vec<String>,
}

fn main() {
    let args: Args = Args::parse();
    let edit_script_path = PathBuf::from(env::var_os("EDIT_SCRIPT").unwrap());
    let edit_script = fs::read_to_string(&edit_script_path).unwrap();

    let mut instructions = edit_script.split('\0').collect_vec();
    if let Some(pos) = instructions.iter().position(|&i| i == "next invocation\n") {
        // Overwrite the edit script. The next time `fake-editor` is called, it will
        // only see the part after the `next invocation` command.
        fs::write(&edit_script_path, instructions[pos + 1..].join("\0")).unwrap();
        instructions.truncate(pos);
    }
    for instruction in instructions {
        let (command, payload) = instruction.split_once('\n').unwrap_or((instruction, ""));
        let parts = command.split(' ').collect_vec();
        match parts.as_slice() {
            [""] => {}
            ["fail"] => exit(1),
            ["dump", dest] => {
                let dest_path = edit_script_path.parent().unwrap().join(dest);
                fs::copy(&args.file, dest_path).unwrap();
            }
            ["dump-path", dest] => {
                let dest_path = edit_script_path.parent().unwrap().join(dest);
                fs::write(&dest_path, args.file.to_str().unwrap())
                    .unwrap_or_else(|err| panic!("Failed to write file {dest_path:?}: {err}"));
            }
            ["expect"] => {
                let actual = String::from_utf8(fs::read(&args.file).unwrap()).unwrap();
                if actual != payload {
                    eprintln!("fake-editor: Unexpected content.\n");
                    eprintln!("EXPECTED: <{payload}>\nRECEIVED: <{actual}>");
                    exit(1)
                }
            }
            ["expect-arg", index] => {
                let index = index.parse::<usize>().unwrap();
                let Some(actual) = args.other_args.get(index) else {
                    eprintln!("fake-editor: Missing argument at index {index}.\n");
                    eprintln!("EXPECTED: <{payload}>");
                    exit(1)
                };

                if actual != payload {
                    eprintln!("fake-editor: Unexpected argument at index {index}.\n");
                    eprintln!("EXPECTED: <{payload}>\nRECEIVED: <{actual}>");
                    exit(1)
                }
            }
            ["write"] => {
                fs::write(&args.file, payload).unwrap_or_else(|_| {
                    panic!("Failed to write file {}", args.file.to_str().unwrap())
                });
            }
            _ => {
                eprintln!("fake-editor: unexpected command: {command}");
                exit(1)
            }
        }
    }
}
