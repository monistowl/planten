use planten_ns::Namespace;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: mount <new> <old>...");
        return;
    }

    let new = &args[1];
    let old: Vec<String> = args[2..].to_vec();

    let mut ns = Namespace::load_from_storage().unwrap_or_else(|e| {
        eprintln!("Error loading namespace: {}", e);
        Namespace::new()
    });

    if old.len() == 1 {
        ns.bind(new, &old[0]);
        println!("bound '{}' to '{}'", new, &old[0]);
    } else {
        let refs: Vec<&str> = old.iter().map(String::as_str).collect();
        ns.union_multi(new, refs.as_slice());
        println!("created union mount at '{}'", new);
    }

    if let Err(e) = ns.save_to_storage() {
        eprintln!("Error saving namespace: {}", e);
    }
}
