use planten_9p::messages::{
    RATTACH, RCLUNK, RERROR, ROPEN, RREAD, RSTAT, RVERSION, RWALK, TATTACH, TCLUNK, TOPEN, TREAD,
    TSTAT, TVERSION, TWALK,
};
use planten_9p::{
    build_frame, decode_string, decode_u16, decode_u32, decode_u64, encode_qid_bytes,
    encode_stat_payload, encode_string, Qid, RawMessage, Stat,
};
use planten_fs_core::FsServer;
use planten_fs_core::Inode;
use std::collections::HashMap;
use std::io::{self, Cursor, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::DevFs;

const MAX_MSG_SIZE: u32 = 8 * 1024;
const VERSION_STRING: &str = "9P2000";

pub fn run_single(listener: TcpListener, fs: Arc<Mutex<DevFs>>) -> io::Result<()> {
    let (stream, _) = listener.accept()?;
    handle_client(stream, fs)
}

struct FidState {
    path: String,
    qid: Qid,
}

fn handle_client(mut stream: TcpStream, fs: Arc<Mutex<DevFs>>) -> io::Result<()> {
    let mut fids: HashMap<u32, FidState> = HashMap::new();

    loop {
        let raw = match RawMessage::read_from(&mut stream) {
            Ok(msg) => msg,
            Err(err) => {
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(err);
            }
        };
        let tag = raw.tag;
        let mut cursor = Cursor::new(raw.body.as_slice());

        match raw.msg_type {
            TVERSION => {
                let msize = decode_u32(&mut cursor)?;
                let version = decode_string(&mut cursor)?;
                let negotiated = msize.min(MAX_MSG_SIZE);
                let mut body = Vec::new();
                body.extend_from_slice(&negotiated.to_le_bytes());
                body.extend_from_slice(&encode_string(if version == VERSION_STRING {
                    VERSION_STRING
                } else {
                    "unknown"
                }));
                send_response(&mut stream, RVERSION, tag, &body)?;
            }
            TATTACH => {
                let fid = decode_u32(&mut cursor)?;
                let _ = decode_u32(&mut cursor)?;
                let _ = decode_string(&mut cursor)?;
                let _ = decode_string(&mut cursor)?;
                let root = root_inode();
                let root_qid = qid_from_inode(&root);
                fids.insert(
                    fid,
                    FidState {
                        path: "/".to_string(),
                        qid: root_qid.clone(),
                    },
                );
                let mut body = Vec::new();
                body.extend_from_slice(&encode_qid_bytes(&root_qid));
                send_response(&mut stream, RATTACH, tag, &body)?;
            }
            TWALK => {
                let fid = decode_u32(&mut cursor)?;
                let newfid = decode_u32(&mut cursor)?;
                let nwname = decode_u16(&mut cursor)?;
                let names: Vec<String> = (0..nwname)
                    .map(|_| decode_string(&mut cursor))
                    .collect::<Result<Vec<String>, _>>()?;

                let fs_locked = fs.lock().unwrap();
                let mut current_path = fids
                    .get(&fid)
                    .map(|state| state.path.clone())
                    .unwrap_or_else(|| "/".to_string());
                let mut qids = Vec::new();
                let mut success = true;

                for name in names {
                    let next_path = resolve_path(&current_path, &name);
                    if let Some(inode) = fs_locked.stat(&next_path) {
                        let qid = qid_from_inode(&inode);
                        qids.push(qid.clone());
                        current_path = next_path;
                    } else {
                        success = false;
                        break;
                    }
                }

                if success {
                    let mut body = Vec::new();
                    body.extend_from_slice(&(qids.len() as u16).to_le_bytes());
                    for qid in &qids {
                        body.extend_from_slice(&encode_qid_bytes(qid));
                    }
                    fids.insert(
                        newfid,
                        FidState {
                            path: current_path,
                            qid: qids.last().cloned().unwrap_or_else(|| root_qid()),
                        },
                    );
                    send_response(&mut stream, RWALK, tag, &body)?;
                } else {
                    send_error(&mut stream, tag, "walk failed")?;
                }
            }
            TREAD => {
                let fid = decode_u32(&mut cursor)?;
                let offset = decode_u64(&mut cursor)?;
                let count = decode_u32(&mut cursor)?;
                if let Some(state) = fids.get(&fid) {
                    let fs_locked = fs.lock().unwrap();
                    if let Some(data) = fs_locked.read(&state.path) {
                        let start = offset as usize;
                        let end = ((offset + count as u64) as usize).min(data.len());
                        let slice = if start < end { &data[start..end] } else { &[] };
                        let mut body = Vec::new();
                        body.extend_from_slice(&(slice.len() as u32).to_le_bytes());
                        body.extend_from_slice(slice);
                        send_response(&mut stream, RREAD, tag, &body)?;
                    } else {
                        send_error(&mut stream, tag, "read failed")?;
                    }
                } else {
                    send_error(&mut stream, tag, "fid not found")?;
                }
            }
            TOPEN => {
                let fid = decode_u32(&mut cursor)?;
                if let Some(state) = fids.get(&fid) {
                    let mut body = Vec::new();
                    body.extend_from_slice(&encode_qid_bytes(&state.qid));
                    body.extend_from_slice(&MAX_MSG_SIZE.to_le_bytes());
                    send_response(&mut stream, ROPEN, tag, &body)?;
                } else {
                    send_error(&mut stream, tag, "fid not known")?;
                }
            }
            TSTAT => {
                let fid = decode_u32(&mut cursor)?;
                if let Some(state) = fids.get(&fid) {
                    let fs_locked = fs.lock().unwrap();
                    if let Some(inode) = fs_locked.stat(&state.path) {
                        let stat = build_stat(&inode);
                        let body = encode_stat_payload(&stat);
                        send_response(&mut stream, RSTAT, tag, &body)?;
                    } else {
                        send_error(&mut stream, tag, "stat failed")?;
                    }
                } else {
                    send_error(&mut stream, tag, "fid not found")?;
                }
            }
            TCLUNK => {
                let fid = decode_u32(&mut cursor)?;
                fids.remove(&fid);
                send_response(&mut stream, RCLUNK, tag, &[])?;
            }
            _ => {
                send_error(&mut stream, tag, "unsupported operation")?;
            }
        }
    }
    Ok(())
}

fn send_response(stream: &mut TcpStream, msg_type: u8, tag: u16, body: &[u8]) -> io::Result<()> {
    let frame = build_frame(msg_type, tag, body);
    stream.write_all(&frame)
}

fn send_error(stream: &mut TcpStream, tag: u16, message: &str) -> io::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(&encode_string(message));
    send_response(stream, RERROR, tag, &body)
}

fn resolve_path(base: &str, name: &str) -> String {
    if base == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", base, name)
    }
}

fn root_inode() -> Inode {
    Inode::new("dev", 0o555 | 0x80000000, "root", "root")
}

fn build_stat(inode: &Inode) -> Stat {
    Stat {
        type_: 0,
        dev: 0,
        qid: qid_from_inode(inode),
        mode: inode.mode,
        atime: inode.atime,
        mtime: inode.mtime,
        length: inode.data.len() as u64,
        name: inode.name.clone(),
        uid: inode.uid.clone(),
        gid: inode.gid.clone(),
        muid: inode.uid.clone(),
    }
}

fn qid_from_inode(inode: &Inode) -> Qid {
    let mut qid = Qid {
        qtype: 0,
        version: 0,
        path: 0,
    };
    if inode.mode & 0x80000000 != 0 {
        qid.qtype = 0x80;
    }
    qid
}

fn root_qid() -> Qid {
    qid_from_inode(&root_inode())
}
