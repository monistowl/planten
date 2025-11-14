use planten_ns::Namespace;
use std::fs;

fn main() {
    let ns_json = fs::read_to_string("/tmp/ns.json").unwrap();
    let ns: Namespace = serde_json::from_str(&ns_json).unwrap();

    println!("{}", serde_json::to_string_pretty(&ns).unwrap());
}