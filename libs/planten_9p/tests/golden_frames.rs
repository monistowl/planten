use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;

use planten_9p::messages::*;
use planten_9p::{RawMessage, decode_qid, decode_stat, decode_string, decode_u32, decode_u64};

fn load_trace(name: &str) -> Vec<u8> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("planten root should exist");
    let path = repo_root.join("tests").join("golden_traces").join(name);
    fs::read(path).expect("failed to read trace")
}

#[test]
fn version_request_trace() {
    let msg = RawMessage::from_bytes(&load_trace("version_t.bin")).unwrap();
    assert_eq!(msg.msg_type, TVERSION);
    assert_eq!(msg.tag, 0);
    let mut cursor = Cursor::new(msg.body.as_slice());
    assert_eq!(decode_u32(&mut cursor).unwrap(), 8192);
    assert_eq!(decode_string(&mut cursor).unwrap(), "9P2000");
}

#[test]
fn version_response_trace() {
    let msg = RawMessage::from_bytes(&load_trace("version_r.bin")).unwrap();
    assert_eq!(msg.msg_type, RVERSION);
    let mut cursor = Cursor::new(msg.body.as_slice());
    assert_eq!(decode_u32(&mut cursor).unwrap(), 131072);
    assert_eq!(decode_string(&mut cursor).unwrap(), "9P2000");
}

#[test]
fn attach_request_trace() {
    let msg = RawMessage::from_bytes(&load_trace("attach_t.bin")).unwrap();
    assert_eq!(msg.msg_type, TATTACH);
    let mut cursor = Cursor::new(msg.body.as_slice());
    assert_eq!(decode_u32(&mut cursor).unwrap(), 1);
    assert_eq!(decode_u32(&mut cursor).unwrap(), 0);
    assert_eq!(decode_string(&mut cursor).unwrap(), "guest");
    assert_eq!(decode_string(&mut cursor).unwrap(), "srv");
}

#[test]
fn attach_response_trace() {
    let msg = RawMessage::from_bytes(&load_trace("attach_r.bin")).unwrap();
    assert_eq!(msg.msg_type, RATTACH);
    let mut cursor = Cursor::new(msg.body.as_slice());
    let qid = decode_qid(&mut cursor).unwrap();
    assert_eq!(qid.qtype, 0);
    assert_eq!(qid.version, 1);
    assert_eq!(qid.path, 0x1234);
}

#[test]
fn read_request_trace() {
    let msg = RawMessage::from_bytes(&load_trace("read_t.bin")).unwrap();
    assert_eq!(msg.msg_type, TREAD);
    let mut cursor = Cursor::new(msg.body.as_slice());
    assert_eq!(decode_u32(&mut cursor).unwrap(), 1);
    assert_eq!(decode_u64(&mut cursor).unwrap(), 0);
    assert_eq!(decode_u32(&mut cursor).unwrap(), 16);
}

#[test]
fn read_response_trace() {
    let msg = RawMessage::from_bytes(&load_trace("read_r.bin")).unwrap();
    assert_eq!(msg.msg_type, RREAD);
    let mut cursor = Cursor::new(msg.body.as_slice());
    let len = decode_u32(&mut cursor).unwrap() as usize;
    let mut data = vec![0u8; len];
    cursor.read_exact(&mut data).unwrap();
    assert_eq!(&data, b"hello");
}

#[test]
fn stat_response_trace() {
    let msg = RawMessage::from_bytes(&load_trace("stat_r.bin")).unwrap();
    assert_eq!(msg.msg_type, RSTAT);
    let mut cursor = Cursor::new(msg.body.as_slice());
    let stat = decode_stat(&mut cursor).unwrap();
    assert_eq!(stat.name, "file");
    assert_eq!(stat.uid, "user");
    assert_eq!(stat.gid, "group");
    assert_eq!(stat.muid, "user");
    assert_eq!(stat.length, 64);
}
