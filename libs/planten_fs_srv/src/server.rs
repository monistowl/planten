use planten_9p::messages::{
    RATTACH, RERROR, ROPEN, RREAD, RSTAT, RVERSION, RWALK, TCLUNK, TSTAT, TVERSION,
};
use planten_9p::{
    build_frame, decode_string, decode_u16, decode_u32, decode_u64, encode_qid_bytes,
    encode_stat_payload, encode_string, Qid, RawMessage, Stat,
};
use planten_fs_core::FsServer;
use std::collections::HashMap;
use std::io::{self, Cursor, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::SrvFs;

const MAX_MSG_SIZE: u32 = 8192;
const VERSION_STRING: &str = "9P2000";

pub fn run_server(listener: TcpListener, fs: Arc<Mutex<SrvFs>>) -> io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let fs = Arc::clone(&fs);
                if let Err(err) = handle_client(stream, fs) {
                    eprintln!("connection error: {}", err);
                }
            }
            Err(err) => eprintln!("accept error: {}", err),
        }
    }
    Ok(())
}

struct FidState {
    path: String,
    qid: Qid,
    open_mode: Option<u8>,
}

fn handle_client(mut stream: TcpStream, fs: Arc<Mutex<SrvFs>>) -> io::Result<()> {
    let mut fids: HashMap<u32, FidState> = HashMap::new();

    loop {
        let raw_message = match RawMessage::read_from(&mut stream) {
            Ok(msg) => msg,
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    break; // Client disconnected
                }
                eprintln!("Error reading raw message: {:?}", e);
                return Err(e);
            }
        };

        let tag = raw_message.tag;
        let mut cursor = Cursor::new(raw_message.body.as_slice());

        let response_body: Option<(u8, Vec<u8>)> = match raw_message.msg_type {
            TVERSION => {
                let msize = decode_u32(&mut cursor)?;
                let version_str = decode_string(&mut cursor)?;
                println!("Tversion: msize={}, version={}", msize, version_str);

                let negotiated_msize = msize.min(MAX_MSG_SIZE);
                let negotiated_version = if version_str == VERSION_STRING {
                    VERSION_STRING
                } else {
                    "unknown"
                };

                let mut body = Vec::new();
                body.extend_from_slice(&negotiated_msize.to_le_bytes());
                body.extend_from_slice(&encode_string(negotiated_version));
                Some((RVERSION, body))
            }
            planten_9p::messages::TATTACH => {
                let fid = decode_u32(&mut cursor)?;
                let _afid = decode_u32(&mut cursor)?; // Not used for now
                let uname = decode_string(&mut cursor)?;
                let aname = decode_string(&mut cursor)?;
                println!("Tattach: fid={}, uname={}, aname={}", fid, uname, aname);

                let root_qid = Qid {
                    qtype: 0x00, // Directory
                    version: 0,
                    path: 0,
                };
                fids.insert(
                    fid,
                    FidState {
                        path: "/".to_string(),
                        qid: root_qid.clone(),
                        open_mode: None,
                    },
                );

                let mut body = Vec::new();
                body.extend_from_slice(&encode_qid_bytes(&root_qid));
                Some((RATTACH, body))
            }
            planten_9p::messages::TWALK => {
                let fid = decode_u32(&mut cursor)?;
                let newfid = decode_u32(&mut cursor)?;
                let nwnames = decode_u16(&mut cursor)?;
                let mut names = Vec::new();
                for _ in 0..nwnames {
                    names.push(decode_string(&mut cursor)?);
                }
                println!("Twalk: fid={}, newfid={}, names={:?}", fid, newfid, names);

                let fs_locked = fs.lock().unwrap();
                let mut qids = Vec::new();
                let mut current_path = fids.get(&fid).map(|f| f.path.clone()).unwrap_or_default();
                let mut walked_successfully = true;

                for name in names {
                    let next_path = if current_path == "/" {
                        format!("/{}", name)
                    } else {
                        format!("{}/{}", current_path, name)
                    };

                    if let Some(inode) = fs_locked.stat(&next_path) {
                        qids.push(Qid {
                            qtype: if inode.mode & 0x80000000 != 0 {
                                0x80 // Directory
                            } else {
                                0x00 // File
                            },
                            version: 0,
                            path: 0, // Placeholder
                        });
                        current_path = next_path;
                    } else {
                        walked_successfully = false;
                        break;
                    }
                }

                if walked_successfully {
                    let mut body = Vec::new();
                    body.extend_from_slice(&(qids.len() as u16).to_le_bytes());
                    for qid in &qids {
                        body.extend_from_slice(&encode_qid_bytes(qid));
                    }
                    fids.insert(
                        newfid,
                        FidState {
                            path: current_path,
                            qid: qids.last().cloned().unwrap_or_else(|| Qid {
                                qtype: 0,
                                version: 0,
                                path: 0,
                            }),
                            open_mode: None,
                        },
                    );
                    Some((RWALK, body))
                } else {
                    Some((RERROR, encode_string("file not found")))
                }
            }
            planten_9p::messages::TOPEN => {
                let fid = decode_u32(&mut cursor)?;
                let mode = raw_message.body[4]; // Mode is the 5th byte
                println!("Topen: fid={}, mode={}", fid, mode);

                let fs_locked = fs.lock().unwrap();
                if let Some(fid_state) = fids.get_mut(&fid) {
                    if let Some(_inode) = fs_locked.stat(&fid_state.path) {
                        fid_state.open_mode = Some(mode);
                        let mut body = Vec::new();
                        body.extend_from_slice(&encode_qid_bytes(&fid_state.qid));
                        body.extend_from_slice(&MAX_MSG_SIZE.to_le_bytes()); // iounit
                        Some((ROPEN, body))
                    } else {
                        Some((RERROR, encode_string("file not found")))
                    }
                } else {
                    Some((RERROR, encode_string("fid not found")))
                }
            }
            planten_9p::messages::TREAD => {
                let fid = decode_u32(&mut cursor)?;
                let offset = decode_u64(&mut cursor)?;
                let count = decode_u32(&mut cursor)?;
                println!("Tread: fid={}, offset={}, count={}", fid, offset, count);

                let fs_locked = fs.lock().unwrap();
                if let Some(fid_state) = fids.get(&fid) {
                    if let Some(data) = fs_locked.read(&fid_state.path) {
                        let start = offset as usize;
                        let end = (offset + count as u64) as usize;
                        let end = end.min(data.len());
                        let slice = if start < end { &data[start..end] } else { &[] };

                        let mut body = Vec::new();
                        body.extend_from_slice(&(slice.len() as u32).to_le_bytes());
                        body.extend_from_slice(slice);
                        Some((RREAD, body))
                    } else {
                        Some((RERROR, encode_string("read failed")))
                    }
                } else {
                    Some((RERROR, encode_string("fid not found")))
                }
            }
            TSTAT => {
                let fid = decode_u32(&mut cursor)?;
                println!("Tstat: fid={}", fid);

                let fs_locked = fs.lock().unwrap();
                if let Some(fid_state) = fids.get(&fid) {
                    if let Some(inode) = fs_locked.stat(&fid_state.path) {
                        let stat = Stat {
                            type_: 0, // Placeholder
                            dev: 0,   // Placeholder
                            qid: fid_state.qid.clone(),
                            mode: inode.mode,
                            atime: inode.atime,
                            mtime: inode.mtime,
                            length: inode.data.len() as u64,
                            name: inode.name,
                            uid: inode.uid,
                            gid: inode.gid,
                            muid: "none".to_string(), // Placeholder
                        };
                        Some((RSTAT, encode_stat_payload(&stat)))
                    } else {
                        Some((RERROR, encode_string("stat failed")))
                    }
                } else {
                    Some((RERROR, encode_string("fid not found")))
                }
            }
            TCLUNK => {
                let fid = decode_u32(&mut cursor)?;
                println!("Tclunk: fid={}", fid);
                fids.remove(&fid);
                Some((planten_9p::messages::RCLUNK, Vec::new()))
            }
            _ => {
                eprintln!("Unhandled message type: {:#x}", raw_message.msg_type);
                Some((RERROR, encode_string("unhandled message type")))
            }
        };

        if let Some((response_type, body)) = response_body {
            let response_frame = build_frame(response_type, tag, &body);
            stream.write_all(&response_frame)?;
        }
    }
    Ok(())
}

pub fn run_single(listener: TcpListener, fs: Arc<Mutex<SrvFs>>) -> io::Result<()> {
    let (stream, _) = listener.accept()?;
    handle_client(stream, fs)
}

pub fn start_server(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("SrvFs 9P server listening on {}", addr);

    let fs = Arc::new(Mutex::new(SrvFs::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New client connected: {:?}", stream.peer_addr());
                let fs_clone = Arc::clone(&fs);
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, fs_clone) {
                        eprintln!("Client handler error: {:?}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {:?}", e);
            }
        }
    }
    Ok(())
}
