use std::env;
use planten_ns::Namespace;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: bind <new> <old>");
        return;
    }

    let new = &args[1];
    let old = &args[2];

    let mut ns = Namespace::new();
    ns.bind(new, old);

    println!("bound '{}' to '{}'", new, old);
}