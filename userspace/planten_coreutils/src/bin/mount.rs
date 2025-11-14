use std::env;
use planten_ns::{Namespace, Mount};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: mount <new> <old>...");
        return;
    }

    let new = &args[1];
    let old: Vec<String> = args[2..].to_vec();

    let mut ns = Namespace::new();
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
}