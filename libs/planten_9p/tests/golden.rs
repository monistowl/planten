use std::fs;
use std::io::{Cursor, Read};

use planten_9p::RawMessage;
use planten_9p::messages::{RATTACH, RERROR, ROPEN, RREAD, RWRITE, RVERSION, RWALK, TATTACH, TVERSION};

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

#[test]
fn golden_version_response_parses() {
    let bytes = fs::read("tests/golden_frames/version_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RVERSION);
    assert_eq!(frame.tag, 0x1234);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let replied_msize = read_u32(&mut cursor);
    assert_eq!(replied_msize, 131_072);

    let version_len = read_u16(&mut cursor) as usize;
    let mut version_buf = vec![0u8; version_len];
    cursor.read_exact(&mut version_buf).unwrap();
    assert_eq!(version_buf, b"9P2000");
}

#[test]
fn golden_walk_response_parses() {
    let bytes = fs::read("tests/golden_frames/walk_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RWALK);
    assert_eq!(frame.tag, 0x3412);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let count = read_u16(&mut cursor);
    assert_eq!(count, 1);

    let mut qid = [0u8; 13];
    cursor.read_exact(&mut qid).unwrap();
    assert_eq!(qid[0], 0);
    assert_eq!(&qid[1..5], &[0, 0, 0, 0]);
    assert_eq!(&qid[5..], &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
}

#[test]
fn golden_open_response_parses() {
    let bytes = fs::read("tests/golden_frames/ropen_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, ROPEN);
    assert_eq!(frame.tag, 0x1234);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let mut qid = [0u8; 13];
    cursor.read_exact(&mut qid).unwrap();
    assert_eq!(qid[0], 0);
    assert_eq!(&qid[5..], &[0, 0, 0, 0, 0, 0, 0, 1]);
    let iounit = read_u32(&mut cursor);
    assert_eq!(iounit, 0x80);
}

#[test]
fn golden_read_response_parses() {
    let bytes = fs::read("tests/golden_frames/rread_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RREAD);
    assert_eq!(frame.tag, 0x5678);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let count = read_u32(&mut cursor);
    assert_eq!(count, 11);
    let mut payload = vec![0u8; count as usize];
    cursor.read_exact(&mut payload).unwrap();
    assert_eq!(&payload, b"hello 9p!!");
}

#[test]
fn golden_error_response_parses() {
    let bytes = fs::read("tests/golden_frames/rerror_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RERROR);
    assert_eq!(frame.tag, 0x2211);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let message_len = read_u16(&mut cursor) as usize;
    let mut buffer = vec![0u8; message_len];
    cursor.read_exact(&mut buffer).unwrap();
    assert_eq!(&buffer, b"oops");
}

#[test]
fn golden_write_response_parses() {
    let bytes = fs::read("tests/golden_frames/rwrite_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RWRITE);
    assert_eq!(frame.tag, 0x3344);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let count = read_u32(&mut cursor);
    assert_eq!(count, 5);
}

#[test]
fn golden_handshake_trace_round_trips() {
    let bytes = fs::read("tests/golden_traces/handshake.bin").unwrap();
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut seen = Vec::new();
    while (cursor.position() as usize) < bytes.len() {
        let frame = RawMessage::read_from(&mut cursor).unwrap();
        seen.push(frame.msg_type);
    }
    assert_eq!(seen, vec![TVERSION, RVERSION, TATTACH, RATTACH]);
}
