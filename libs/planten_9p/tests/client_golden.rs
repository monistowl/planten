use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::thread;

use planten_9p::P9Client;
use planten_9p::RawMessage;
use planten_9p::messages::{RATTACH, RVERSION, TATTACH, TVERSION};

fn load_frames(path: &str) -> Vec<(Vec<u8>, RawMessage)> {
    let bytes = fs::read(path).unwrap();
    let mut cursor = std::io::Cursor::new(&bytes);
    let mut frames = Vec::new();
    while (cursor.position() as usize) < bytes.len() {
        let frame = RawMessage::read_from(&mut cursor).unwrap();
        let consumed = frame.size as usize;
        let start = (cursor.position() as usize) - consumed;
        let chunk = bytes[start..start + consumed].to_vec();
        frames.push((chunk, frame));
    }
    frames
}

#[test]
fn golden_handshake_streams() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = channel();
    let frames = load_frames("tests/golden_traces/handshake.bin");

    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut stream = stream;
        for (chunk, frame) in frames {
            match frame.msg_type {
                TVERSION | TATTACH => {
                    let incoming = RawMessage::read_from(&mut stream).unwrap();
                    assert_eq!(incoming.msg_type, frame.msg_type);
                }
                RVERSION | RATTACH => {
                    stream.write_all(&chunk).unwrap();
                }
                _ => {}
            }
        }
        tx.send(()).unwrap();
    });

    let mut client = P9Client::new(&addr.to_string()).unwrap();
    let version = client.version(8192, "9P2000").unwrap();
    assert_eq!(version, "9P2000");
    client.attach(0, None, "guest", "").unwrap();
    rx.recv().unwrap();
}
