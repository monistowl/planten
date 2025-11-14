
use planten_fs_ramfs::RamFs;
use planten_fs_core::FsServer;
use std::net::TcpListener;
use std::io::{Read, Write};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:5640").unwrap();
    let mut ramfs = RamFs::new();

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let mut buf = [0; 1024];
        let n = stream.read(&mut buf).unwrap();
        let req = String::from_utf8_lossy(&buf[..n]);
        let mut parts = req.split_whitespace();
        let cmd = parts.next().unwrap();
        let path = parts.next().unwrap_or("");

        match cmd {
            "read" => {
                if let Some(data) = ramfs.read(path) {
                    stream.write_all(data).unwrap();
                }
            }
            _ => {
                stream.write_all(b"unknown command").unwrap();
            }
        }
    }
}
