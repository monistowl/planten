
use planten_ns::{Namespace, Mount};

fn main() {
    let mut ns = Namespace::new();
    ns.bind("/bin", "/usr/bin");
    ns.union("/lib", "/usr/lib");
    ns.union("/lib", "/lib64");

    println!("{}", serde_json::to_string_pretty(&ns).unwrap());
}
