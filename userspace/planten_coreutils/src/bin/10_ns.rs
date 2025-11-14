
use std::env;
use std::process::Command;
use planten_ns::{Namespace, Mount};
use nix::unistd::{fork, ForkResult, execvp};
use nix::sched::{unshare, CloneFlags};
use nix::mount::{mount, MsFlags};
use std::ffi::CString;
use tempfile::tempdir;
use std::io::{self, Write};
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut ns = Namespace::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-b" => {
                ns.bind(&args[i+1], &args[i+2]);
                i += 3;
            }
            "-u" => {
                ns.union(&args[i+1], &args[i+2]);
                i += 3;
            }
            _ => {
                break;
            }
        }
    }

    let mut file = File::create("/tmp/ns.json").unwrap();
    file.write_all(serde_json::to_string_pretty(&ns).unwrap().as_bytes()).unwrap();

    let cmd_args = &args[i..];
    if cmd_args.is_empty() {
        // basic shell
        loop {
            print!("> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).unwrap() == 0 {
                break;
            }

            let input = input.trim();
            if input == "exit" {
                break;
            }

            let mut parts = input.split_whitespace();
            let command = parts.next().unwrap();
            let args = parts;

            let mut cmd = Command::new(command);
            cmd.args(args);

            let status = cmd.status().expect("failed to execute command");
            if !status.success() {
                eprintln!("command failed: {}", status);
            }
        }
    } else {
        let cmd = &cmd_args[0];
        let c_cmd = CString::new(cmd.as_bytes()).unwrap();
        let c_args: Vec<CString> = cmd_args.iter().map(|s| CString::new(s.as_bytes()).unwrap()).collect();

        match unsafe{fork()} {
            Ok(ForkResult::Parent { child, .. }) => {
                println!("child pid: {}", child);
            }
            Ok(ForkResult::Child) => {
                unshare(CloneFlags::CLONE_NEWNS).unwrap();
                for (new, old) in ns.mounts() {
                    match old {
                        Mount::Bind{path} => {
                            mount(Some(path.as_str()), new.as_str(), None, MsFlags::MS_BIND, None).unwrap();
                        }
                        Mount::Union{paths} => {
                            let tmp_dir = tempdir().unwrap();
                            for path in paths {
                                let target = tmp_dir.path().join(path.split('/').last().unwrap());
                                mount(Some(path.as_str()), target.to_str().unwrap(), None, MsFlags::MS_BIND, None).unwrap();
                            }
                            mount(Some(tmp_dir.path().to_str().unwrap()), new.as_str(), None, MsFlags::MS_BIND, None).unwrap();
                        }
                    }
                }
                execvp(&c_cmd, &c_args).unwrap();
            }
            Err(_) => {
                println!("Fork failed");
            }
        }
    }
}
