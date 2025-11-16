pub mod messages;

use std::io::{self, Cursor, Read, Write};
use std::net::TcpStream;

use crate::messages::*;

/// Raw 9P frame.
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qid {
    pub qtype: u8,
    pub version: u32,
    pub path: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub type_: u16,
    pub dev: u32,
    pub qid: Qid,
    pub mode: u32,
    pub atime: u32,
    pub mtime: u32,
    pub length: u64,
    pub name: String,
    pub uid: String,
    pub gid: String,
    pub muid: String,
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

pub fn encode_version_body(msize: u32, version: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + 2 + version.len());
    buf.extend_from_slice(&msize.to_le_bytes());
    buf.extend_from_slice(&(version.len() as u16).to_le_bytes());
    buf.extend_from_slice(version.as_bytes());
    buf
}

pub fn encode_auth_body(fid: u32, uname: &str, aname: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&encode_string(uname));
    buf.extend_from_slice(&encode_string(aname));
    buf
}

pub fn encode_attach_body(fid: u32, afid: Option<u32>, uname: &str, aname: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&afid.unwrap_or(0).to_le_bytes());
    buf.extend_from_slice(&encode_string(uname));
    buf.extend_from_slice(&encode_string(aname));
    buf
}

pub fn encode_walk_body(fid: u32, newfid: u32, names: &[&str]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&newfid.to_le_bytes());
    buf.extend_from_slice(&(names.len() as u16).to_le_bytes());
    for name in names {
        buf.extend_from_slice(&encode_string(name));
    }
    buf
}

pub fn encode_open_body(fid: u32, mode: u8) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.push(mode);
    buf
}

pub fn encode_read_body(fid: u32, offset: u64, count: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&offset.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf
}

pub fn encode_write_body(fid: u32, offset: u64, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&offset.to_le_bytes());
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

pub fn encode_clunk_body(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

pub fn encode_create_body(fid: u32, name: &str, perm: u32, mode: u8) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&encode_string(name));
    buf.extend_from_slice(&perm.to_le_bytes());
    buf.push(mode);
    buf
}

pub fn encode_remove_body(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

pub fn encode_stat_body(fid: u32) -> Vec<u8> {
    fid.to_le_bytes().to_vec()
}

pub fn encode_wstat_body(fid: u32, stat: &Stat) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&encode_stat_payload(stat));
    buf
}

pub fn encode_clone_body(fid: u32, newfid: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&fid.to_le_bytes());
    buf.extend_from_slice(&newfid.to_le_bytes());
    buf
}

pub fn encode_flush_body(oldtag: u16) -> Vec<u8> {
    oldtag.to_le_bytes().to_vec()
}

pub fn encode_stat_payload(stat: &Stat) -> Vec<u8> {
    let mut stat_buf = Vec::new();
    stat_buf.extend_from_slice(&stat.type_.to_le_bytes());
    stat_buf.extend_from_slice(&stat.dev.to_le_bytes());
    stat_buf.extend_from_slice(&encode_qid_bytes(&stat.qid));
    stat_buf.extend_from_slice(&stat.mode.to_le_bytes());
    stat_buf.extend_from_slice(&stat.atime.to_le_bytes());
    stat_buf.extend_from_slice(&stat.mtime.to_le_bytes());
    stat_buf.extend_from_slice(&stat.length.to_le_bytes());
    stat_buf.extend_from_slice(&encode_string(&stat.name));
    stat_buf.extend_from_slice(&encode_string(&stat.uid));
    stat_buf.extend_from_slice(&encode_string(&stat.gid));
    stat_buf.extend_from_slice(&encode_string(&stat.muid));
    let mut payload = Vec::new();
    payload.extend_from_slice(&(stat_buf.len() as u16).to_le_bytes());
    payload.extend_from_slice(&stat_buf);
    payload
}

pub fn encode_string(value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(value.len() as u16).to_le_bytes());
    buf.extend_from_slice(value.as_bytes());
    buf
}

pub fn encode_qid_bytes(qid: &Qid) -> [u8; 13] {
    let mut buf = [0u8; 13];
    buf[0] = qid.qtype;
    buf[1..5].copy_from_slice(&qid.version.to_le_bytes());
    buf[5..13].copy_from_slice(&qid.path.to_le_bytes());
    buf
}

pub fn decode_string(cursor: &mut Cursor<&[u8]>) -> io::Result<String> {
    let len = decode_u16(cursor)? as usize;
    let mut buffer = vec![0u8; len];
    cursor.read_exact(&mut buffer)?;
    String::from_utf8(buffer)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8 string"))
}

pub fn decode_u16(cursor: &mut Cursor<&[u8]>) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn decode_u32(cursor: &mut Cursor<&[u8]>) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn decode_u64(cursor: &mut Cursor<&[u8]>) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    cursor.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

pub fn decode_qid(cursor: &mut Cursor<&[u8]>) -> io::Result<Qid> {
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
        qtype,
        version,
        path,
    })
}

pub fn decode_stat(cursor: &mut Cursor<&[u8]>) -> io::Result<Stat> {
    let stat_size = decode_u16(cursor)? as usize;
    let mut buffer = vec![0u8; stat_size];
    cursor.read_exact(&mut buffer)?;
    let mut inner = Cursor::new(buffer.as_slice());
    let type_ = decode_u16(&mut inner)?;
    let dev = decode_u32(&mut inner)?;
    let qid = decode_qid(&mut inner)?;
    let mode = decode_u32(&mut inner)?;
    let atime = decode_u32(&mut inner)?;
    let mtime = decode_u32(&mut inner)?;
    let length = decode_u64(&mut inner)?;
    let name = decode_string(&mut inner)?;
    let uid = decode_string(&mut inner)?;
    let gid = decode_string(&mut inner)?;
    let muid = decode_string(&mut inner)?;
    Ok(Stat {
        type_,
        dev,
        qid,
        mode,
        atime,
        mtime,
        length,
        name,
        uid,
        gid,
        muid,
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
        let mut cursor = Cursor::new(msg.body.as_slice());
        assert_eq!(decode_u32(&mut cursor).unwrap(), 8192);
        assert_eq!(decode_string(&mut cursor).unwrap(), "9P2000");
    }

    #[test]
    fn encode_and_decode_string() {
        let value = "hello";
        let encoded = encode_string(value);
        let mut cursor = Cursor::new(encoded.as_slice());
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
        let mut cursor = Cursor::new(msg.body.as_slice());
        let count = decode_u16(&mut cursor).unwrap();
        assert_eq!(count, 1);
        let qid = decode_qid(&mut cursor).unwrap();
        assert_eq!(qid.qtype, 0u8);
    }
}
