use std::convert::TryInto;
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::RawMessage;
use planten_9p::messages::{RCLONE, RERROR, RSTAT, TATTACH, TCLONE, TFLUSH, TREAD, TWALK};
use planten_fs_ramfs::{RamFs, server};

const CLIENT_SESSION: &str = "libs/planten_9p/tests/golden_traces/client_session.bin";

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

fn read_u16(cursor: &mut Cursor<&[u8]>) -> u16 {
    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf).unwrap();
    u16::from_le_bytes(buf)
}

fn read_u32(cursor: &mut Cursor<&[u8]>) -> u32 {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf).unwrap();
    u32::from_le_bytes(buf)
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> u64 {
    let mut buf = [0u8; 8];
    cursor.read_exact(&mut buf).unwrap();
    u64::from_le_bytes(buf)
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> io::Result<String> {
    let len = read_u16(cursor)? as usize;
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid string"))
}

fn is_request(msg: u8) -> bool {
    matches!(
        msg,
        100 | TATTACH | TWALK | TREAD | TWRITE | TREMOVE | TCLONE | TSTAT | TWSTAT | TFLUSH
    )
}

fn run_sequence_against(addr: &str, frames: &[(Vec<u8>, RawMessage)]) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;
    for (chunk, frame) in frames {
        if is_request(frame.msg_type) {
            stream.write_all(chunk)?;
        } else {
            let response = RawMessage::read_from(&mut stream)?;
            assert_eq!(response.msg_type, frame.msg_type);
        }
    }
    Ok(())
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

    let dir_request =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/tread_dir_request.bin").unwrap());
    stream.write_all(&dir_request[0].0).unwrap();
    let actual_dir = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_dir.msg_type, RREAD);
    let mut dir_cursor = Cursor::new(actual_dir.body.as_slice());
    let count = read_u32(&mut dir_cursor);
    assert_eq!(count, 24);
    let mut dir_buf = vec![0u8; count as usize];
    dir_cursor.read_exact(&mut dir_buf).unwrap();
    assert_eq!(&dir_buf, b"hello.txt\nreadme.txt\n");

    let tstat_request =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/tstat_request.bin").unwrap());
    stream.write_all(&tstat_request[0].0).unwrap();
    let actual_stat = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_stat.msg_type, RSTAT);
    let expected_stat = RawMessage::from_bytes(
        &fs::read("../planten_9p/tests/golden_frames/rstat_response.bin").unwrap(),
    )
    .unwrap();
    assert_eq!(actual_stat.body, expected_stat.body);
    let mut stat_cursor = Cursor::new(actual_stat.body.as_slice());
    let _stat_size = read_u16(&mut stat_cursor);
    let _stat_type = read_u16(&mut stat_cursor);
    let _stat_dev = read_u32(&mut stat_cursor);
    let _ = stat_cursor.read_exact(&mut [0u8; 13]);
    let _mode = read_u32(&mut stat_cursor);
    let _atime = read_u32(&mut stat_cursor);
    let _mtime = read_u32(&mut stat_cursor);
    let _length = read_u64(&mut stat_cursor);
    let name = read_string(&mut stat_cursor).unwrap();
    assert_eq!(name, "hello.txt");

    let write_exchange =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/write_exchange.bin").unwrap());
    stream.write_all(&write_exchange[0].0).unwrap();
    let actual_write = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_write.msg_type, write_exchange[1].1.msg_type);
    assert_eq!(actual_write.body, write_exchange[1].1.body);

    let twstat_request =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/twstat_request.bin").unwrap());
    stream.write_all(&twstat_request[0].0).unwrap();
    let actual_rwstat = RawMessage::read_from(&mut stream).unwrap();
    let expected_rwstat = RawMessage::from_bytes(
        &fs::read("../planten_9p/tests/golden_frames/rwstat_response.bin").unwrap(),
    )
    .unwrap();
    assert_eq!(actual_rwstat.msg_type, expected_rwstat.msg_type);
    assert_eq!(actual_rwstat.body, expected_rwstat.body);

    let remove_exchange =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/remove_exchange.bin").unwrap());
    stream.write_all(&remove_exchange[0].0).unwrap();
    let actual_remove = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_remove.msg_type, remove_exchange[1].1.msg_type);

    for (req_path, resp_path) in &[
        (
            "../planten_9p/tests/golden_traces/twalk_error_request.bin",
            "../planten_9p/tests/golden_traces/rerror_walk.bin",
        ),
        (
            "../planten_9p/tests/golden_traces/twalk_multi_request.bin",
            "../planten_9p/tests/golden_traces/rerror_walk_multi.bin",
        ),
    ] {
        let error_walk = parse_frames(&fs::read(req_path).unwrap());
        stream.write_all(&error_walk[0].0).unwrap();
        let actual_walk_error = RawMessage::read_from(&mut stream).unwrap();
        let expected_walk_error = RawMessage::from_bytes(&fs::read(resp_path).unwrap()).unwrap();
        assert_eq!(actual_walk_error.msg_type, expected_walk_error.msg_type);
        assert_eq!(actual_walk_error.body, expected_walk_error.body);
    }

    let flush_request =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/tflush_request.bin").unwrap());
    stream.write_all(&flush_request[0].0).unwrap();
    let actual_flush = RawMessage::read_from(&mut stream).unwrap();
    let expected_flush = RawMessage::from_bytes(
        &fs::read("../planten_9p/tests/golden_traces/rflush_response.bin").unwrap(),
    )
    .unwrap();
    assert_eq!(actual_flush.msg_type, expected_flush.msg_type);
    assert_eq!(actual_flush.body, expected_flush.body);

    let clone_request =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/tclone_request.bin").unwrap());
    stream.write_all(&clone_request[0].0).unwrap();
    let actual_clone = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_clone.msg_type, RCLONE);
    assert_eq!(actual_clone.tag, 0x9999);

    let flush_error_request = parse_frames(
        &fs::read("../planten_9p/tests/golden_traces/tflush_error_request.bin").unwrap(),
    );
    stream.write_all(&flush_error_request[0].0).unwrap();
    let actual_flush_error = RawMessage::read_from(&mut stream).unwrap();
    let expected_flush_error = RawMessage::from_bytes(
        &fs::read("../planten_9p/tests/golden_traces/rflush_error.bin").unwrap(),
    )
    .unwrap();
    assert_eq!(actual_flush_error.msg_type, expected_flush_error.msg_type);
    assert_eq!(actual_flush_error.body, expected_flush_error.body);

    let oob_request =
        parse_frames(&fs::read("../planten_9p/tests/golden_traces/tread_oob_request.bin").unwrap());
    stream.write_all(&oob_request[0].0).unwrap();
    let actual_oob = RawMessage::read_from(&mut stream).unwrap();
    let expected_oob = RawMessage::from_bytes(
        &fs::read("../planten_9p/tests/golden_traces/rerror_oob.bin").unwrap(),
    )
    .unwrap();
    assert_eq!(actual_oob.msg_type, expected_oob.msg_type);
    assert_eq!(actual_oob.body, expected_oob.body);

    // After removal the same read request should produce an error
    stream.write_all(&read_exchange[0].0).unwrap();
    let actual_error = RawMessage::read_from(&mut stream).unwrap();
    assert_eq!(actual_error.msg_type, RERROR);

    drop(stream);
    server_thread.join().unwrap();
}

#[test]
fn client_session_against_ramfs() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let ramfs = Arc::new(Mutex::new(RamFs::new()));
    {
        let mut guard = ramfs.lock().unwrap();
        guard.create_file("/hello.txt", b"hello 9p!!");
    }

    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || {
        server::run_single(listener, server_ramfs).unwrap();
    });

    let frames = parse_frames(&fs::read(CLIENT_SESSION).unwrap());
    run_sequence_against(&addr.to_string(), &frames).unwrap();

    server_thread.join().unwrap();
}
