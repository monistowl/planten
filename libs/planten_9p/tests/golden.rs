use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;

use planten_9p::RawMessage;
use planten_9p::decode_stat;
use planten_9p::messages::{
    RATTACH, RCLONE, RERROR, ROPEN, RREAD, RSTAT, RVERSION, RWALK, RWRITE, TATTACH, TCLONE, TREAD,
    TSTAT, TVERSION, TWALK,
};

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

fn repo_trace_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .join("tests")
        .join("golden_traces")
        .join(file)
}

fn read_trace_messages(file: &str) -> Vec<RawMessage> {
    let bytes = fs::read(repo_trace_path(file)).unwrap();
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut frames = Vec::new();
    while (cursor.position() as usize) < bytes.len() {
        frames.push(RawMessage::read_from(&mut cursor).unwrap());
    }
    frames
}

#[test]
fn golden_version_response_parses() {
    let bytes = fs::read(repo_trace_path("version_r.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RVERSION);
    assert_eq!(frame.tag, 0);
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
    let bytes = fs::read(repo_trace_path("walk_response.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RWALK);
    assert_eq!(frame.tag, 0x0002);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let count = read_u16(&mut cursor);
    assert_eq!(count, 1);

    let mut qid = [0u8; 13];
    cursor.read_exact(&mut qid).unwrap();
    assert_eq!(qid[0], 0);
    assert_eq!(&qid[1..5], &[0, 0, 0, 0]);
    assert_eq!(&qid[5..], &[0x62, 0x3c, 0x92, 0xa9, 0xce, 0xc3, 0x4d, 0x3c]);
}

#[test]
fn golden_open_response_parses() {
    let bytes = fs::read(repo_trace_path("ropen_root_response.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, ROPEN);
    assert_eq!(frame.tag, 0x0005);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let mut qid = [0u8; 13];
    cursor.read_exact(&mut qid).unwrap();
    assert_eq!(qid[0], 0x80);
    let iounit = read_u32(&mut cursor);
    assert_eq!(iounit, 0);
}

#[test]
fn golden_read_response_parses() {
    let bytes = fs::read(repo_trace_path("read_r.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RREAD);
    assert_eq!(frame.tag, 0x0002);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let count = read_u32(&mut cursor);
    assert_eq!(count, 5);
    let mut payload = vec![0u8; count as usize];
    cursor.read_exact(&mut payload).unwrap();
    assert_eq!(&payload, b"hello");
}

#[test]
fn golden_error_response_parses() {
    let bytes = fs::read(repo_trace_path("rerror_oob.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RERROR);
    assert_eq!(frame.tag, 0x0010);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let message_len = read_u16(&mut cursor) as usize;
    let mut buffer = vec![0u8; message_len];
    cursor.read_exact(&mut buffer).unwrap();
    assert_eq!(&buffer, b"unknown fid");
}

#[test]
fn golden_write_response_parses() {
    let frames = read_trace_messages("write_exchange.bin");
    assert_eq!(frames.len(), 2);
    let frame = &frames[1];
    assert_eq!(frame.msg_type, RWRITE);
    assert_eq!(frame.tag, frames[0].tag);

    let mut cursor = Cursor::new(frame.body.as_slice());
    let count = read_u32(&mut cursor);
    assert_eq!(count, 11);
}

#[test]
fn golden_handshake_trace_round_trips() {
    let bytes = fs::read(repo_trace_path("handshake.bin")).unwrap();
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut seen = Vec::new();
    while (cursor.position() as usize) < bytes.len() {
        let frame = RawMessage::read_from(&mut cursor).unwrap();
        seen.push(frame.msg_type);
    }
    assert_eq!(seen, vec![TVERSION, RVERSION, TATTACH, RATTACH]);
}

#[test]
fn golden_clone_trace_parses() {
    let bytes = fs::read(repo_trace_path("tclone_request.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, TCLONE);
    assert_eq!(frame.tag, 0x9999);
    assert_eq!(frame.size as usize, bytes.len());

    let bytes = fs::read(repo_trace_path("rclone_response.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RCLONE);
    assert_eq!(frame.tag, 0x9999);
}

#[test]
fn golden_twalk_error_parses() {
    let bytes = fs::read(repo_trace_path("twalk_error_request.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, TWALK);
    assert_eq!(frame.tag, 0x000b);

    let bytes = fs::read(repo_trace_path("rerror_walk.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RERROR);
    assert_eq!(frame.tag, 0x000b);
}

#[test]
fn golden_tread_oob_error_parses() {
    let bytes = fs::read(repo_trace_path("tread_oob_request.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, TREAD);
    assert_eq!(frame.tag, 0x0010);

    let bytes = fs::read(repo_trace_path("rerror_oob.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RERROR);
    assert_eq!(frame.tag, 0x0010);
}

#[test]
fn golden_stat_response_parses() {
    let bytes = fs::read(repo_trace_path("rstat_response.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RSTAT);
    assert_eq!(frame.tag, 0x0007);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let stat = decode_stat(&mut cursor).unwrap();
    assert_eq!(stat.name, "hello.txt");
    assert_eq!(stat.mode & 0o777, 0o644);
    assert_eq!(stat.length, 10);
}

#[test]
fn golden_tstat_error_trace_matches() {
    let bytes = fs::read(repo_trace_path("tstat_error_request.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, TSTAT);
    assert_eq!(frame.tag, 0x000f);

    let bytes = fs::read(repo_trace_path("rerror_tstat.bin")).unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RERROR);
    assert_eq!(frame.tag, 0x000f);

    let mut cursor = Cursor::new(frame.body.as_slice());
    let message_len = read_u16(&mut cursor) as usize;
    let mut buffer = vec![0u8; message_len];
    cursor.read_exact(&mut buffer).unwrap();
    assert_eq!(&buffer, b"unknown fid");
}
