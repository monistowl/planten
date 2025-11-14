use std::io;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use planten_fs_ramfs::{RamFs, server};

const LISTEN_ADDR: &str = "127.0.0.1:5640";

fn main() -> io::Result<()> {
    let listener = TcpListener::bind(LISTEN_ADDR)?;
    let ramfs = Arc::new(Mutex::new(RamFs::new()));

    {
        let mut guard = ramfs.lock().unwrap();
        guard.create_file("/hello.txt", b"hello 9p!!");
        guard.create_file("/readme.txt", b"RAMFS as a 9P server");
    }

    println!("planten_fs_ramfs 9P server listening on {}", LISTEN_ADDR);
    server::run_server(listener, ramfs)
}
