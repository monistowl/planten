pub mod messages;

use std::io::{self, Cursor, Read, Write};
use std::net::TcpStream;

use crate::messages::*;

/// Raw 9P frame.
#[derive(Debug)]
pub struct RawMessage {
    pub size: u32,
    pub msg_type: u8,
    pub tag: u16,
    pub body: Vec<u8>,
}

impl RawMessage {
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut size_bytes = [0u8; 4];
        reader.read_exact(&mut size_bytes)?;
        let size = u32::from_le_bytes(size_bytes);
        if size < 7 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "message header too small",
            ));
        }

        let mut header = [0u8; 3];
        reader.read_exact(&mut header)?;

        let msg_type = header[0];
        let tag = u16::from_le_bytes([header[1], header[2]]);

        let body_len = size as usize - 7;
        let mut body = vec![0u8; body_len];
        if body_len > 0 {
            reader.read_exact(&mut body)?;
        }

        Ok(RawMessage {
            size,
            msg_type,
            tag,
            body,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        Self::read_from(&mut cursor)
    }
}

/// Lightweight 9P client that can negotiate, attach, walk, open, read, and clunk.
pub struct P9Client {
    stream: TcpStream,
    next_tag: u16,
}

impl P9Client {
    pub fn new(addr: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        Ok(P9Client {
            stream,
            next_tag: 0,
        })
    }

    fn next_tag(&mut self) -> u16 {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1);
        tag
    }

    fn send_and_wait(&mut self, msg_type: u8, body: &[u8]) -> io::Result<RawMessage> {
        let tag = self.next_tag();
        let frame = build_frame(msg_type, tag, body);
        self.stream.write_all(&frame)?;
        let response = RawMessage::read_from(&mut self.stream)?;
        if response.tag != tag {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "mismatched tag"));
        }
        Ok(response)
    }

    pub fn version(&mut self, msize: u32, version_str: &str) -> io::Result<String> {
        let body = encode_version_body(msize, version_str);
        let response = self.send_and_wait(TVERSION, &body)?;
        ensure_msg_type(&response, RVERSION)?;
        let mut cursor = Cursor::new(response.body.as_slice());
        let negotiated = decode_u32(&mut cursor)?;
        let negotiated_version = decode_string(&mut cursor)?;
        if negotiated < 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid negotiated msize",
            ));
        }
        Ok(negotiated_version)
    }

    pub fn attach(
        &mut self,
        fid: u32,
        afid: Option<u32>,
        uname: &str,
        aname: &str,
    ) -> io::Result<()> {
        let body = encode_attach_body(fid, afid, uname, aname);
        let response = self.send_and_wait(TATTACH, &body)?;
        ensure_msg_type(&response, RATTACH)?;
        Ok(())
    }

    pub fn walk(&mut self, fid: u32, newfid: u32, names: &[&str]) -> io::Result<usize> {
        let body = encode_walk_body(fid, newfid, names);
        let response = self.send_and_wait(TWALK, &body)?;
        ensure_msg_type(&response, RWALK)?;
        let mut cursor = Cursor::new(response.body.as_slice());
        let nwqid = decode_u16(&mut cursor)? as usize;
        for _ in 0..nwqid {
            decode_qid(&mut cursor)?;
        }
        Ok(nwqid)
    }

    pub fn open(&mut self, fid: u32, mode: u8) -> io::Result<u32> {
        let body = encode_open_body(fid, mode);
        let response = self.send_and_wait(TOPEN, &body)?;
        ensure_msg_type(&response, ROPEN)?;
        let mut cursor = Cursor::new(response.body.as_slice());
        decode_qid(&mut cursor)?;
        let iounit = decode_u32(&mut cursor)?;
        Ok(iounit)
    }

    pub fn read(&mut self, fid: u32, offset: u64, count: u32) -> io::Result<Vec<u8>> {
        let body = encode_read_body(fid, offset, count);
        let response = self.send_and_wait(TREAD, &body)?;
        ensure_msg_type(&response, RREAD)?;
        let mut cursor = Cursor::new(response.body.as_slice());
        let data_len = decode_u32(&mut cursor)? as usize;
        let mut payload = vec![0u8; data_len];
        cursor.read_exact(&mut payload)?;
        Ok(payload)
    }

    pub fn clunk(&mut self, fid: u32) -> io::Result<()> {
        let body = encode_clunk_body(fid);
        let response = self.send_and_wait(TCLUNK, &body)?;
        ensure_msg_type(&response, RCLUNK)?;
        Ok(())
    }
}

fn ensure_msg_type(response: &RawMessage, expected: u8) -> io::Result<()> {
    if response.msg_type != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unexpected message type {:#x}, expected {:#x}",
                response.msg_type, expected
            ),
        ));
    }
    Ok(())
}

pub fn build_frame(msg_type: u8, tag: u16, body: &[u8]) -> Vec<u8> {
    let size = 7 + body.len() as u32;
    let mut buffer = Vec::with_capacity(size as usize);
    buffer.extend_from_slice(&size.to_le_bytes());
    buffer.push(msg_type);
    buffer.extend_from_slice(&tag.to_le_bytes());
    buffer.extend_from_slice(body);
    buffer
}

fn encode_version_body(msize: u32, version: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + 2 + version.len());
    buf.extend_from_slice(&msize.to_le_bytes());
    buf.extend_from_slice(&u16::try_from(version.len()).unwrap_or(0).to_le_bytes());
    buf.extend_from_slice(version.as_bytes());
    buf
}

fn encode_attach_body(fid: u32, afid: Option<u32>, uname: &str, aname: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&afid.unwrap_or(0).to_le_bytes());
    buf.extend_from_slice(&encode_string(uname));
    buf.extend_from_slice(&encode_string(aname));
    buf
}

fn encode_walk_body(fid: u32, newfid: u32, names: &[&str]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&newfid.to_le_bytes());
    buf.extend_from_slice(&(names.len() as u16).to_le_bytes());
    for name in names {
        buf.extend_from_slice(&encode_string(name));
    }
    buf
}

fn encode_open_body(fid: u32, mode: u8) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.push(mode);
    buf
}

fn encode_read_body(fid: u32, offset: u64, count: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&offset.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf
}

fn encode_clunk_body(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

fn encode_string(value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(value.len() as u16).to_le_bytes());
    buf.extend_from_slice(value.as_bytes());
    buf
}

fn decode_string(cursor: &mut Cursor<&[u8]>) -> io::Result<String> {
    let len = decode_u16(cursor)? as usize;
    let mut buffer = vec![0u8; len];
    cursor.read_exact(&mut buffer)?;
    match String::from_utf8(buffer) {
        Ok(s) => Ok(s),
        Err(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid UTF-8 string",
        )),
    }
}

fn decode_u16(cursor: &mut Cursor<&[u8]>) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn decode_u32(cursor: &mut Cursor<&[u8]>) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

#[derive(Debug)]
struct Qid {
    _qtype: u8,
    _version: u32,
    _path: u64,
}

fn decode_qid(cursor: &mut Cursor<&[u8]>) -> io::Result<Qid> {
    let mut qid_buf = [0u8; 13];
    cursor.read_exact(&mut qid_buf)?;
    let qtype = qid_buf[0];
    let version = u32::from_le_bytes([qid_buf[1], qid_buf[2], qid_buf[3], qid_buf[4]]);
    let path = u64::from_le_bytes([
        qid_buf[5],
        qid_buf[6],
        qid_buf[7],
        qid_buf[8],
        qid_buf[9],
        qid_buf[10],
        qid_buf[11],
        qid_buf[12],
    ]);
    Ok(Qid {
        _qtype: qtype,
        _version: version,
        _path: path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_parse_version_message() {
        let body = encode_version_body(8192, "9P2000");
        let frame = build_frame(TVERSION, 7, &body);
        let msg = RawMessage::from_bytes(&frame).expect("parsable message");
        assert_eq!(msg.msg_type, TVERSION);
        assert_eq!(msg.tag, 7);
        let mut cursor = Cursor::new(&msg.body);
        assert_eq!(decode_u32(&mut cursor).unwrap(), 8192);
        assert_eq!(decode_string(&mut cursor).unwrap(), "9P2000");
    }

    #[test]
    fn encode_and_decode_string() {
        let value = "hello";
        let encoded = encode_string(value);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_string(&mut cursor).unwrap(), "hello");
    }

    #[test]
    fn walk_response_skips_qids() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(1u16).to_le_bytes());
        buf.extend_from_slice(&[0u8]); // qid flags
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        let frame = build_frame(RWALK, 1, &buf);
        let msg = RawMessage::from_bytes(&frame).unwrap();
        let mut cursor = Cursor::new(&msg.body);
        let count = decode_u16(&mut cursor).unwrap();
        assert_eq!(count, 1);
        let qid = decode_qid(&mut cursor).unwrap();
        assert_eq!(qid._qtype, 0u8);
    }
}
