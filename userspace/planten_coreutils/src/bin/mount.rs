use planten_ns::{Mount, Namespace};
use std::env;

const NAMESPACE_FILE: &str = ".planten_namespace.json";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: mount <new> <old>...");
        return;
    }

    let new = &args[1];
    let old: Vec<String> = args[2..].to_vec();

    let mut ns = Namespace::load_from_file(NAMESPACE_FILE).unwrap_or_else(|e| {
        eprintln!("Error loading namespace: {}", e);
        Namespace::new()
    });

    if old.len() == 1 {
        ns.bind(new, &old[0]);
        println!("bound '{}' to '{}'", new, &old[0]);
    } else {
        let mut union_mount = Mount::Union { paths: vec![] };
        if let Mount::Union { paths } = &mut union_mount {
            for path in old {
                paths.push(path);
            }
        }
        ns.add_mount(new, union_mount);
        println!("created union mount at '{}'", new);
    }

    if let Err(e) = ns.save_to_file(NAMESPACE_FILE) {
        eprintln!("Error saving namespace: {}", e);
    }
}
