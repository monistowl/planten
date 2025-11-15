use std::fs;
use std::io::{Cursor, Read};

use planten_9p::RawMessage;
use planten_9p::messages::{
    RATTACH, RCLONE, RERROR, ROPEN, RREAD, RSTAT, RVERSION, RWALK, RWRITE, TATTACH, TREAD, TVERSION, TWALK,
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

#[test]
fn golden_clone_trace_parses() {
    let bytes = fs::read("tests/golden_traces/tclone_request.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, TCLONE);
    assert_eq!(frame.tag, 0x9999);
    assert_eq!(frame.size as usize, bytes.len());

    let bytes = fs::read("tests/golden_traces/rclone_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RCLONE);
    assert_eq!(frame.tag, 0x9999);
}

#[test]
fn golden_twalk_error_parses() {
    let bytes = fs::read("tests/golden_traces/twalk_error_request.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, TWALK);
    assert_eq!(frame.tag, 0xaadd);

    let bytes = fs::read("tests/golden_traces/rerror_walk.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RERROR);
    assert_eq!(frame.tag, 0xaadd);
}

#[test]
fn golden_tread_oob_error_parses() {
    let bytes = fs::read("tests/golden_traces/tread_oob_request.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, TREAD);
    assert_eq!(frame.tag, 0x0202);

    let bytes = fs::read("tests/golden_traces/rerror_oob.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RERROR);
    assert_eq!(frame.tag, 0x0202);
}

#[test]
fn golden_stat_response_parses() {
    let bytes = fs::read("tests/golden_frames/rstat_response.bin").unwrap();
    let frame = RawMessage::from_bytes(&bytes).unwrap();
    assert_eq!(frame.msg_type, RSTAT);
    assert_eq!(frame.tag, 0x4321);
    assert_eq!(frame.size as usize, bytes.len());

    let mut cursor = Cursor::new(frame.body.as_slice());
    let stat_size = read_u16(&mut cursor);
    let mut stat_buf = vec![0u8; stat_size as usize];
    cursor.read_exact(&mut stat_buf).unwrap();

    let mut stat_cursor = Cursor::new(stat_buf.as_slice());
    let _type = read_u16(&mut stat_cursor);
    let _dev = read_u32(&mut stat_cursor);
    let mut qid = [0u8; 13];
    stat_cursor.read_exact(&mut qid).unwrap();
    let mode = read_u32(&mut stat_cursor);
    let _atime = read_u32(&mut stat_cursor);
    let _mtime = read_u32(&mut stat_cursor);
    let length = u64::from_le_bytes(stat_buf[30..38].try_into().unwrap());
    
    assert_eq!(mode, 0o755 | 0x80000000);
    assert_eq!(length, 0);
}

