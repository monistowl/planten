use std::convert::TryInto;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::RawMessage;
use planten_fs_ramfs::{RamFs, server};

fn parse_frames(bytes: &[u8]) -> Vec<(Vec<u8>, RawMessage)> {
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let size = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let chunk = bytes[pos..pos + size].to_vec();
        let raw = RawMessage::from_bytes(&chunk).unwrap();
        frames.push((chunk, raw));
        pos += size;
    }
    frames
}

#[test]
fn golden_trace_matches_server_interaction() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let ramfs = Arc::new(Mutex::new(RamFs::new()));
    {
        let mut guard = ramfs.lock().unwrap();
        guard.create_file("/hello.txt", b"hello 9p!!");
        guard.create_file("/readme.txt", b"RAMFS as a 9P server");
    }

    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || {
        server::run_single(listener, server_ramfs).unwrap();
    });

    let mut stream = TcpStream::connect(addr).unwrap();

    let handshake_frames =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/handshake.bin").unwrap());
    let request_indices = [0, 2];
    let response_indices = [1, 3];

    for &idx in &request_indices {
        stream.write_all(&handshake_frames[idx].0).unwrap();
    }

    for &idx in &response_indices {
        let actual = RawMessage::read_from(&mut stream).unwrap();
        let expected = &handshake_frames[idx].1;
        assert_eq!(actual.msg_type, expected.msg_type);
        assert_eq!(actual.body, expected.body);
    }

    let walk_request =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/twalk_request.bin").unwrap());
    assert_eq!(walk_request.len(), 1);
    stream.write_all(&walk_request[0].0).unwrap();

    let expected_walk = RawMessage::from_bytes(
        &fs::read("../planten_9p/tests/golden_frames/walk_response.bin").unwrap(),
    )
    .unwrap();
    let actual_walk = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_walk.msg_type, expected_walk.msg_type);
    assert_eq!(actual_walk.body, expected_walk.body);

    let read_exchange =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/read_exchange.bin").unwrap());
    stream.write_all(&read_exchange[0].0).unwrap();
    let actual_read = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_read.msg_type, read_exchange[1].1.msg_type);
    assert_eq!(actual_read.body, read_exchange[1].1.body);

    let write_exchange =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/write_exchange.bin").unwrap());
    stream.write_all(&write_exchange[0].0).unwrap();
    let actual_write = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_write.msg_type, write_exchange[1].1.msg_type);
    assert_eq!(actual_write.body, write_exchange[1].1.body);

    drop(stream);
    server_thread.join().unwrap();
}
