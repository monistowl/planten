use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use planten_9p::{RawMessage, build_frame, messages::*};
use planten_fs_core::FsServer;

use crate::RamFs;

pub fn run_server(listener: TcpListener, ramfs: Arc<Mutex<RamFs>>) -> io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let ramfs = Arc::clone(&ramfs);
                if let Err(err) = handle_client(stream, ramfs) {
                    eprintln!("connection error: {}", err);
                }
            }
            Err(err) => eprintln!("accept error: {}", err),
        }
    }
    Ok(())
}

pub fn run_single(listener: TcpListener, ramfs: Arc<Mutex<RamFs>>) -> io::Result<()> {
    let (stream, _) = listener.accept()?;
    handle_client(stream, ramfs)
}

pub fn handle_client(mut stream: TcpStream, ramfs: Arc<Mutex<RamFs>>) -> io::Result<()> {
    let mut fid_paths: HashMap<u32, String> = HashMap::new();

    loop {
        let message = match RawMessage::read_from(&mut stream) {
            Ok(msg) => msg,
            Err(err) => {
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    return Ok(());
                }
                return Err(err);
            }
        };

        match message.msg_type {
            TVERSION => handle_version(&mut stream, message.tag, &message.body)?,
            TATTACH => handle_attach(&mut stream, message.tag, &message.body, &mut fid_paths)?,
            TWALK => handle_walk(
                &mut stream,
                message.tag,
                &message.body,
                &mut fid_paths,
                &ramfs,
            )?,
            TOPEN => handle_open(&mut stream, message.tag, &message.body, &fid_paths, &ramfs)?,
            TREAD => handle_read(&mut stream, message.tag, &message.body, &fid_paths, &ramfs)?,
            TWRITE => handle_write(&mut stream, message.tag, &message.body, &fid_paths, &ramfs)?,
            TCLUNK => handle_clunk(&mut stream, message.tag, &message.body, &mut fid_paths)?,
            _ => send_error(&mut stream, message.tag, "unsupported message")?,
        }
    }
}

fn handle_version(stream: &mut TcpStream, tag: u16, body: &[u8]) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let _msize = read_u32(&mut cursor)?;
    let _version = read_string(&mut cursor)?;
    let response = build_version_body(131072, "9P2000");
    send_response(stream, RVERSION, tag, &response)
}

fn handle_attach(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_paths: &mut HashMap<u32, String>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let _afid = read_u32(&mut cursor)?;
    let _uname = read_string(&mut cursor)?;
    let _aname = read_string(&mut cursor)?;
    fid_paths.insert(fid, "/".to_string());
    let mut response = Vec::new();
    response.extend_from_slice(&encode_qid("/"));
    send_response(stream, RATTACH, tag, &response)
}

fn handle_walk(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_paths: &mut HashMap<u32, String>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let newfid = read_u32(&mut cursor)?;
    let nwname = read_u16(&mut cursor)?;

    let base_path = fid_paths
        .get(&fid)
        .cloned()
        .unwrap_or_else(|| "/".to_string());
    let mut current_path = base_path.clone();
    let mut qids: Vec<String> = Vec::new();

    for _ in 0..nwname {
        let name = read_string(&mut cursor)?;
        match resolve_step(&current_path, &name) {
            Some(next_path) => {
                if path_exists(&next_path, ramfs) {
                    current_path = next_path.clone();
                    qids.push(next_path);
                } else {
                    return send_error(
                        stream,
                        tag,
                        &format!("walk failed: component '{}' not found", name),
                    );
                }
            }
            None => {
                return send_error(
                    stream,
                    tag,
                    &format!("walk failed: invalid component '{}'", name),
                );
            }
        }
    }

    fid_paths.insert(newfid, current_path.clone());

    let mut response = Vec::new();
    response.extend_from_slice(&(qids.len() as u16).to_le_bytes());
    for path in qids {
        response.extend_from_slice(&encode_qid(&path));
    }
    send_response(stream, RWALK, tag, &response)
}

fn handle_open(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_paths: &HashMap<u32, String>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let _mode = read_u8(&mut cursor)?;

    let path = match fid_paths.get(&fid) {
        Some(path) => path,
        None => {
            return send_error(stream, tag, "unknown fid");
        }
    };

    if !path_exists(path, ramfs) {
        return send_error(stream, tag, "file not found");
    }

    let mut response = Vec::new();
    response.extend_from_slice(&encode_qid(path));
    response.extend_from_slice(&0u32.to_le_bytes());
    send_response(stream, ROPEN, tag, &response)
}

fn handle_read(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_paths: &HashMap<u32, String>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let offset = read_u64(&mut cursor)?;
    let count = read_u32(&mut cursor)?;

    let path = match fid_paths.get(&fid) {
        Some(path) => path,
        None => {
            return send_error(stream, tag, "unknown fid");
        }
    };

    let data = {
        let guard = ramfs.lock().unwrap();
        guard.read_file(path).map(|bytes| bytes.to_vec())
    };

    let data = match data {
        Some(buf) => {
            let start = std::cmp::min(offset as usize, buf.len());
            let end = std::cmp::min(start + count as usize, buf.len());
            buf[start..end].to_vec()
        }
        None => {
            return send_error(stream, tag, "cannot read directory or missing file");
        }
    };

    let mut response = Vec::new();
    response.extend_from_slice(&(data.len() as u32).to_le_bytes());
    response.extend_from_slice(&data);
    send_response(stream, RREAD, tag, &response)
}

fn handle_write(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_paths: &HashMap<u32, String>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let _offset = read_u64(&mut cursor)?;
    let count = read_u32(&mut cursor)?;
    let mut buffer = vec![0u8; count as usize];
    cursor.read_exact(&mut buffer)?;

    let path = match fid_paths.get(&fid) {
        Some(path) => path,
        None => {
            return send_error(stream, tag, "unknown fid");
        }
    };

    {
        let mut guard = ramfs.lock().unwrap();
        guard.write(path, &buffer);
    }

    let mut response = Vec::new();
    response.extend_from_slice(&count.to_le_bytes());
    send_response(stream, RWRITE, tag, &response)
}

fn handle_clunk(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_paths: &mut HashMap<u32, String>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    fid_paths.remove(&fid);
    send_response(stream, RCLUNK, tag, &[])
}

fn send_response(stream: &mut TcpStream, msg_type: u8, tag: u16, body: &[u8]) -> io::Result<()> {
    let frame = build_frame(msg_type, tag, body);
    stream.write_all(&frame)
}

fn send_error(stream: &mut TcpStream, tag: u16, message: &str) -> io::Result<()> {
    let body = encode_error(message);
    send_response(stream, RERROR, tag, &body)
}

fn build_version_body(msize: u32, version: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&msize.to_le_bytes());
    buf.extend_from_slice(&(version.len() as u16).to_le_bytes());
    buf.extend_from_slice(version.as_bytes());
    buf
}

fn encode_error(message: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(message.len() as u16).to_le_bytes());
    buf.extend_from_slice(message.as_bytes());
    buf
}

fn encode_qid(path: &str) -> [u8; 13] {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let path_id = hasher.finish();

    let mut qid = [0u8; 13];
    qid[0] = 0;
    qid[1..5].copy_from_slice(&0u32.to_le_bytes());
    qid[5..13].copy_from_slice(&path_id.to_le_bytes());
    qid
}

fn path_exists(path: &str, ramfs: &Arc<Mutex<RamFs>>) -> bool {
    let guard = ramfs.lock().unwrap();
    guard.read_file(path).is_some() || guard.list_dir(path).is_some()
}

fn resolve_step(base: &str, component: &str) -> Option<String> {
    if component.is_empty() {
        return None;
    }

    match component {
        "." => Some(base.to_string()),
        "/" => Some("/".to_string()),
        ".." => {
            if base == "/" {
                Some("/".to_string())
            } else {
                let trimmed = base.trim_end_matches('/');
                match trimmed.rfind('/') {
                    Some(idx) if idx == 0 => Some("/".to_string()),
                    Some(idx) => Some(trimmed[..idx].to_string()),
                    None => Some("/".to_string()),
                }
            }
        }
        _ => {
            if base == "/" {
                Some(format!("/{}", component))
            } else {
                Some(format!("{}/{}", base, component))
            }
        }
    }
}

fn read_u8(cursor: &mut Cursor<&[u8]>) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16(cursor: &mut Cursor<&[u8]>) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32(cursor: &mut Cursor<&[u8]>) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    cursor.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> io::Result<String> {
    let len = read_u16(cursor)? as usize;
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid string"))
}
