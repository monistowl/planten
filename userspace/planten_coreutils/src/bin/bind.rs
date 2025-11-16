use planten_ns::Namespace;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: bind <new> <old>");
        return;
    }

    let new = &args[1];
    let old = &args[2];

    let mut ns = Namespace::load_from_storage().unwrap_or_else(|e| {
        eprintln!("Error loading namespace: {}", e);
        Namespace::new()
    });
    ns.bind(new, old);

    if let Err(e) = ns.save_to_storage() {
        eprintln!("Error saving namespace: {}", e);
    }

    println!("bound '{}' to '{}'", new, old);
}
