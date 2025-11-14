
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
use std::collections::HashMap;
use std::net::TcpStream;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut ns = Namespace::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-b" => {
                if i + 2 >= args.len() {
                    eprintln!("-b requires two arguments");
                    return;
                }
                ns.bind(&args[i+1], &args[i+2]);
                i += 3;
            }
            "-u" => {
                if i + 2 >= args.len() {
                    eprintln!("-u requires two arguments");
                    return;
                }
                ns.union(&args[i+1], &args[i+2]);
                i += 3;
            }
            "-p9" => {
                if i + 3 >= args.len() {
                    eprintln!("-p9 requires three arguments: <new> <addr> <path>");
                    return;
                }
                ns.p9(&args[i+1], &args[i+2], &args[i+3]);
                i += 4;
            }
            _ => {
                break;
            }
        }
    }

    if let Err(e) = File::create("/tmp/ns.json")
        .and_then(|mut file| file.write_all(serde_json::to_string_pretty(&ns).unwrap().as_bytes()))
    {
        eprintln!("Failed to write namespace file: {}", e);
    }

    let cmd_args = &args[i..];
    if cmd_args.is_empty() {
        // basic shell
        let mut builtins: HashMap<String, fn(Vec<&str>, &mut Namespace)> = HashMap::new();
        builtins.insert("bind".to_string(), |args, ns| {
            if args.len() != 2 {
                eprintln!("usage: bind <new> <old>");
                return;
            }
            ns.bind(args[0], args[1]);
        });
        builtins.insert("mount".to_string(), |args, ns| {
            if args.len() < 2 {
                eprintln!("usage: mount <new> <old>...");
                return;
            }
            let new = args[0];
            let old = &args[1..];
            if old.len() == 1 {
                ns.bind(new, old[0]);
            } else {
                let mut union_mount = Mount::Union { paths: vec![] };
                if let Mount::Union { paths } = &mut union_mount {
                    for path in old {
                        paths.push(path.to_string());
                    }
                }
                ns.add_mount(new, union_mount);
            }
        });
        builtins.insert("nsctl".to_string(), |_, ns| {
            println!("{}", serde_json::to_string_pretty(&ns).unwrap());
        });

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
            let command = match parts.next() {
                Some(cmd) => cmd,
                None => continue,
            };
            let args: Vec<&str> = parts.collect();

            if let Some(builtin) = builtins.get(command) {
                builtin(args, &mut ns);
                if let Err(e) = File::create("/tmp/ns.json")
                    .and_then(|mut file| file.write_all(serde_json::to_string_pretty(&ns).unwrap().as_bytes()))
                {
                    eprintln!("Failed to write namespace file: {}", e);
                }
            } else {
                let mut cmd = Command::new(command);
                cmd.args(args);

                match cmd.status() {
                    Ok(status) => {
                        if !status.success() {
                            eprintln!("command failed: {}", status);
                        }
                    }
                    Err(e) => {
                        eprintln!("failed to execute command: {}", e);
                    }
                }
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
                if let Err(e) = unshare(CloneFlags::CLONE_NEWNS) {
                    eprintln!("Failed to unshare namespace: {}", e);
                    return;
                }
                for (new, old) in ns.mounts() {
                    match old {
                        Mount::Bind{path} => {
                            if let Err(e) = mount(Some(path.as_str()), new.as_str(), None, MsFlags::MS_BIND, None) {
                                eprintln!("Failed to bind mount {} to {}: {}", path, new, e);
                            }
                        }
                        Mount::Union{paths} => {
                            let tmp_dir = match tempfile::tempdir() {
                                Ok(dir) => dir,
                                Err(e) => {
                                    eprintln!("Failed to create temp dir: {}", e);
                                    return;
                                }
                            };
                            for path in paths {
                                let target = tmp_dir.path().join(path.split('/').last().unwrap());
                                if let Err(e) = mount(Some(path.as_str()), target.to_str().unwrap(), None, MsFlags::MS_BIND, None) {
                                    eprintln!("Failed to bind mount {} to {:?}: {}", path, target, e);
                                }
                            }
                            if let Err(e) = mount(Some(tmp_dir.path().to_str().unwrap()), new.as_str(), None, MsFlags::MS_BIND, None) {
                                eprintln!("Failed to bind mount {:?} to {}: {}", tmp_dir.path(), new, e);
                            }
                        }
                        Mount::P9{addr, path} => {
                            let mut stream = match TcpStream::connect(addr) {
                                Ok(stream) => stream,
                                Err(e) => {
                                    eprintln!("Failed to connect to 9P server at {}: {}", addr, e);
                                    return;
                                }
                            };
                            // For now, just print a message
                            println!("Connected to 9P server at {} for path {}", addr, path);
                            // TODO: Implement 9P protocol and actual mount
                        }
                    }
                }
                if let Err(e) = execvp(&c_cmd, &c_args) {
                    eprintln!("Failed to exec command: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Fork failed: {}", e);
            }
        }
    }
}
