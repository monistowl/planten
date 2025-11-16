use planten_ns::{Mount, Namespace};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        eprintln!("usage: nsctl");
        return;
    }

    match Namespace::load_from_storage() {
        Ok(ns) => {
            if ns.mounts().is_empty() {
                println!("No mounts in namespace.");
            } else {
                println!("Current Namespace:");
                for entry in ns.mounts() {
                    match &entry.mount {
                        Mount::Bind { path: old_path } => {
                            println!("  {} -> (bind) {}", entry.target, old_path);
                        }
                        Mount::Union { paths } => {
                            println!("  {} -> (union) {:?}", entry.target, paths);
                        }
                        Mount::P9 {
                            addr,
                            path: p9_path,
                        } => {
                            println!("  {} -> (p9) {}@{}", entry.target, p9_path, addr);
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error loading namespace: {}", e);
            eprintln!("A new namespace will be created on the first 'bind' or 'mount' operation.");
        }
    }
}
