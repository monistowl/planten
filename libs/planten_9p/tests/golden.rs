use std::fs;
use std::io::{Cursor, Read};

use planten_9p::RawMessage;
use planten_9p::messages::{RVERSION, RWALK};

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
    assert_eq!(frame.tag, 0x3412); // 0x1234 little-end? Actually header tag bytes 0x34,0x12 -> value 0x1234? need check? Received from file: header [msg_type=111=RWALK, tag bytes 0x12,0x34]. little-end -> 0x3412? Wait we set bytes list to 111,0x12,0x34 but we might have tag 0x3412? we want 0x3412 or 0x1234? Maybe we targeted 0x3412? we used 0x12 0x34 so little-end -> 0x3412. We'll check accordingly.
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
