#[cfg(target_os = "linux")]
use nix::mount::{MsFlags, mount};
#[cfg(target_os = "linux")]
use nix::sched::{CloneFlags, unshare};
use nix::unistd::{ForkResult, execvp, fork};
#[cfg(target_os = "linux")]
use planten_9p::P9Client;
use planten_ns::{Mount, Namespace};
use std::collections::HashMap;
use std::env;
use std::ffi::CString;
#[cfg(target_os = "linux")]
use std::fs;
use std::fs::File;
use std::io::{self, Write};
#[cfg(target_os = "linux")]
use std::path::Path;
use std::process::Command;
#[cfg(target_os = "linux")]
use tempfile::tempdir;

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
                ns.bind(&args[i + 1], &args[i + 2]);
                i += 3;
            }
            "-u" => {
                if i + 2 >= args.len() {
                    eprintln!("-u requires two arguments");
                    return;
                }
                ns.union(&args[i + 1], &args[i + 2]);
                i += 3;
            }
            "-p9" => {
                if i + 3 >= args.len() {
                    eprintln!("-p9 requires three arguments: <new> <addr> <path>");
                    return;
                }
                ns.p9(&args[i + 1], &args[i + 2], &args[i + 3]);
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
                if let Err(e) = File::create("/tmp/ns.json").and_then(|mut file| {
                    file.write_all(serde_json::to_string_pretty(&ns).unwrap().as_bytes())
                }) {
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
        let c_args: Vec<CString> = cmd_args
            .iter()
            .map(|s| CString::new(s.as_bytes()).unwrap())
            .collect();

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                println!("child pid: {}", child);
            }
            Ok(ForkResult::Child) => {
                #[cfg(target_os = "linux")]
                if let Err(e) = unshare(CloneFlags::CLONE_NEWNS) {
                    eprintln!("Failed to unshare namespace: {}", e);
                    return;
                }
                #[cfg(not(target_os = "linux"))]
                {
                    eprintln!("Skipping namespace isolation: not available on this platform");
                }
                for (new, old) in ns.mounts() {
                    #[cfg(target_os = "linux")]
                    {
                        match old {
                            Mount::Bind { path } => {
                                if let Err(e) = mount(
                                    Some(path.as_str()),
                                    new.as_str(),
                                    None,
                                    MsFlags::MS_BIND,
                                    None,
                                ) {
                                    eprintln!("Failed to bind mount {} to {}: {}", path, new, e);
                                }
                            }
                            Mount::Union { paths } => {
                                let tmp_dir = match tempdir() {
                                    Ok(dir) => dir,
                                    Err(e) => {
                                        eprintln!("Failed to create temp dir: {}", e);
                                        return;
                                    }
                                };
                                for path in paths {
                                    let target =
                                        tmp_dir.path().join(path.split('/').last().unwrap());
                                    if let Err(e) = mount(
                                        Some(path.as_str()),
                                        target.to_str().unwrap(),
                                        None,
                                        MsFlags::MS_BIND,
                                        None,
                                    ) {
                                        eprintln!(
                                            "Failed to bind mount {} to {:?}: {}",
                                            path, target, e
                                        );
                                    }
                                }
                                if let Err(e) = mount(
                                    Some(tmp_dir.path().to_str().unwrap()),
                                    new.as_str(),
                                    None,
                                    MsFlags::MS_BIND,
                                    None,
                                ) {
                                    eprintln!(
                                        "Failed to bind mount {:?} to {}: {}",
                                        tmp_dir.path(),
                                        new,
                                        e
                                    );
                                }
                            }
                            Mount::P9 { addr, path } => {
                                if let Err(err) = probe_remote_share(addr, path) {
                                    eprintln!("Failed to probe 9P {}@{}: {}", path, addr, err);
                                    continue;
                                }
                                if let Err(e) = mount_9p_target(new, addr, path) {
                                    eprintln!(
                                        "Failed to mount 9P {}@{} onto {}: {}",
                                        path, addr, new, e
                                    );
                                } else {
                                    println!("Mounted 9P {}@{} onto {}", path, addr, new);
                                }
                            }
                        }
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        let _ = old;
                        eprintln!("Skipping mount {}: Linux-only host support", new);
                    }
                }
                #[allow(irrefutable_let_patterns)]
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

#[cfg(target_os = "linux")]
fn mount_9p_target(target: &str, addr: &str, remote_path: &str) -> Result<(), String> {
    ensure_mount_point(target).map_err(|e| format!("invalid mount point {}: {}", target, e))?;

    let remote = if remote_path.is_empty() {
        "/"
    } else {
        remote_path
    };
    let (host, port) = parse_9p_addr(addr)?;

    let source = host.clone();
    let mut options = format!("trans=tcp,port={},aname={}", port, remote);
    options.push_str(",msize=131072,cache=loose");

    mount(
        Some(source.as_str()),
        target,
        Some("9p"),
        MsFlags::empty(),
        Some(options.as_str()),
    )
    .map_err(|e| format!("mount system call failed: {}", e))
}

#[cfg(target_os = "linux")]
fn probe_remote_share(addr: &str, remote_path: &str) -> Result<(), String> {
    let mut client =
        P9Client::new(addr).map_err(|e| format!("failed to connect to {}: {}", addr, e))?;

    let uname = std::env::var("USER").unwrap_or_else(|_| "guest".to_string());
    client
        .version(131072, "9P2000")
        .map_err(|e| format!("version exchange failed: {}", e))?;
    client
        .attach(0, None, uname.as_str(), "")
        .map_err(|e| format!("attach failed: {}", e))?;

    let components: Vec<&str> = remote_path.split('/').filter(|s| !s.is_empty()).collect();
    if !components.is_empty() {
        client
            .walk(0, 1, components.as_slice())
            .map_err(|e| format!("walk failed: {}", e))?;
        client.clunk(1).ok();
    }

    client
        .clunk(0)
        .map_err(|e| format!("clunk root fid failed: {}", e))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_mount_point(target: &str) -> io::Result<()> {
    let mount_point = Path::new(target);
    if mount_point.exists() {
        if mount_point.is_dir() {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} exists and is not a directory", target),
        ));
    }
    fs::create_dir_all(mount_point)
}

#[cfg(target_os = "linux")]
const DEFAULT_9P_PORT: u16 = 564;

#[cfg(target_os = "linux")]
fn parse_9p_addr(addr: &str) -> Result<(String, u16), String> {
    if addr.trim().is_empty() {
        return Err("address is empty".to_string());
    }

    if addr.contains('!') {
        let parts: Vec<&str> = addr.split('!').filter(|s| !s.is_empty()).collect();
        if parts.len() < 2 {
            return Err(format!("invalid 9P addr '{}'", addr));
        }
        let port_part = parts.last().unwrap();
        let host_part = parts[parts.len() - 2];
        let port = port_part
            .parse::<u16>()
            .map_err(|_| format!("invalid port '{}' in addr '{}'", port_part, addr))?;
        return Ok((host_part.to_string(), port));
    }

    if addr.starts_with('[') {
        if let Some(end_idx) = addr.find(']') {
            let host = addr[1..end_idx].to_string();
            if addr.len() > end_idx + 1 {
                let remainder = &addr[end_idx + 1..];
                if remainder.starts_with(':') && remainder.len() > 1 {
                    let port_str = &remainder[1..];
                    let port = port_str
                        .parse::<u16>()
                        .map_err(|_| format!("invalid port '{}' in addr '{}'", port_str, addr))?;
                    return Ok((host, port));
                }
            }
            return Ok((host, DEFAULT_9P_PORT));
        } else {
            return Err(format!("invalid IPv6 addr '{}'", addr));
        }
    }

    let colon_count = addr.matches(':').count();
    if colon_count == 1 {
        if let Some(idx) = addr.rfind(':') {
            let host = addr[..idx].to_string();
            let port_str = &addr[idx + 1..];
            let port = port_str
                .parse::<u16>()
                .map_err(|_| format!("invalid port '{}' in addr '{}'", port_str, addr))?;
            return Ok((host, port));
        }
    }

    Ok((addr.to_string(), DEFAULT_9P_PORT))
}
