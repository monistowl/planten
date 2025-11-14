use planten_ns::Namespace;
use std::env;

const NAMESPACE_FILE: &str = ".planten_namespace.json";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        eprintln!("usage: nsctl");
        return;
    }

    match Namespace::load_from_file(NAMESPACE_FILE) {
        Ok(ns) => {
            if ns.mounts().is_empty() {
                println!("No mounts in namespace.");
            } else {
                println!("Current Namespace:");
                for (path, mount) in ns.mounts() {
                    match mount {
                        planten_ns::Mount::Bind { path: old_path } => {
                            println!("  {} -> (bind) {}", path, old_path);
                        }
                        planten_ns::Mount::Union { paths } => {
                            println!("  {} -> (union) {:?}", path, paths);
                        }
                        planten_ns::Mount::P9 {
                            addr,
                            path: p9_path,
                        } => {
                            println!("  {} -> (p9) {}@{}", path, p9_path, addr);
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error loading namespace from {}: {}", NAMESPACE_FILE, e);
            eprintln!("A new namespace will be created on the first 'bind' or 'mount' operation.");
        }
    }
}
