use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::{build_frame, messages::*, RawMessage};
use planten_fs_core::FsServer;

use crate::RamFs;

#[derive(Clone)]
struct FidState {
    path: String,
    qid: [u8; 13],
    open_mode: Option<u8>,
}

impl FidState {
    fn new(path: String, qid: [u8; 13]) -> Self {
        FidState {
            path,
            qid,
            open_mode: None,
        }
    }
}

pub fn run_server(listener: TcpListener, ramfs: Arc<Mutex<RamFs>>) -> io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let ramfs = Arc::clone(&ramfs);
                thread::spawn(move || {
                    if let Err(err) = handle_client(stream, ramfs) {
                        eprintln!("connection error: {}", err);
                    }
                });
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
    let mut fid_states: HashMap<u32, FidState> = HashMap::new();

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
            TATTACH => handle_attach(&mut stream, message.tag, &message.body, &mut fid_states)?,
            TWALK => handle_walk(
                &mut stream,
                message.tag,
                &message.body,
                &mut fid_states,
                &ramfs,
            )?,
            TOPEN => handle_open(
                &mut stream,
                message.tag,
                &message.body,
                &mut fid_states,
                &ramfs,
            )?,
            TREAD => handle_read(&mut stream, message.tag, &message.body, &fid_states, &ramfs)?,
            TWRITE => handle_write(&mut stream, message.tag, &message.body, &fid_states, &ramfs)?,
            TWSTAT => handle_wstat(&mut stream, message.tag, &message.body, &fid_states, &ramfs)?,
            TFLUSH => handle_flush(&mut stream, message.tag, &message.body)?,
            TREMOVE => handle_remove(
                &mut stream,
                message.tag,
                &message.body,
                &mut fid_states,
                &ramfs,
            )?,
            TCLUNK => handle_clunk(&mut stream, message.tag, &message.body, &mut fid_states)?,
            TSTAT => handle_stat(&mut stream, message.tag, &message.body, &fid_states, &ramfs)?,
            TCLONE => handle_clone(&mut stream, message.tag, &message.body, &mut fid_states)?,
            TCREATE => handle_create(
                &mut stream,
                message.tag,
                &message.body,
                &mut fid_states,
                &ramfs,
            )?,
            TAUTH => handle_auth(&mut stream, message.tag, &message.body)?,
            _ => send_error(&mut stream, message.tag, "unsupported message")?,
        }
    }
}

fn handle_auth(stream: &mut TcpStream, tag: u16, _body: &[u8]) -> io::Result<()> {
    // For now, we don't support authentication, but we need to reply
    // with a valid Rauth message to allow clients to connect without auth.
    // The aqid should represent a file on which read/write operations
    // can be performed to complete the authentication protocol.
    // Since we don't have one, we'll send a dummy qid.
    let aqid = [0u8; 13]; // A dummy qid.
    send_response(stream, RAUTH, tag, &aqid)
}

fn handle_create(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &mut HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let name = read_string(&mut cursor)?;
    let perm = read_u32(&mut cursor)?;
    let _mode = read_u8(&mut cursor)?;

    let path = match fid_states.get(&fid) {
        Some(state) => &state.path,
        None => {
            return send_error(stream, tag, "unknown fid");
        }
    };

    let new_path = resolve_step(path, &name).unwrap();

    let mut guard = ramfs.lock().unwrap();
    if guard.read_file(&new_path).is_some() || guard.list_dir(&new_path).is_some() {
        return send_error(stream, tag, "file exists");
    }

    if perm & 0x80000000 != 0 {
        // DMDIR
        guard.create_dir(&new_path);
    } else {
        guard.create_file(&new_path, &[]);
    }

    let mut response = Vec::new();
    response.extend_from_slice(&encode_qid(&new_path));
    response.extend_from_slice(&0u32.to_le_bytes());
    send_response(stream, RCREATE, tag, &response)
}

fn handle_clone(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &mut HashMap<u32, FidState>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let newfid = read_u32(&mut cursor)?;

    let state = match fid_states.get(&fid) {
        Some(state) => state.clone(),
        None => {
            return send_error(stream, tag, "unknown fid");
        }
    };

    fid_states.insert(newfid, state);

    send_response(stream, RCLONE, tag, &[])
}

fn handle_stat(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;

    let path = match fid_states.get(&fid) {
        Some(state) => &state.path,
        None => {
            return send_error(stream, tag, "unknown fid");
        }
    };

    let guard = ramfs.lock().unwrap();
    let inode = match guard.stat(path) {
        Some(inode) => inode,
        None => {
            return send_error(stream, tag, "file not found");
        }
    };

    let stat = build_stat(
        &inode.name,
        inode.data.len() as u64,
        inode.mode,
        &inode.uid,
        &inode.gid,
        inode.atime,
        inode.mtime,
    );

    send_response(stream, RSTAT, tag, &stat)
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
    fid_states: &mut HashMap<u32, FidState>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let _afid = read_u32(&mut cursor)?;
    let _uname = read_string(&mut cursor)?;
    let _aname = read_string(&mut cursor)?;
    let root_path = "/".to_string();
    let root_qid = encode_qid(&root_path);
    fid_states.insert(fid, FidState::new(root_path.clone(), root_qid));
    let mut response = Vec::new();
    response.extend_from_slice(&encode_qid("/"));
    send_response(stream, RATTACH, tag, &response)
}

fn handle_walk(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &mut HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let newfid = read_u32(&mut cursor)?;
    let nwname = read_u16(&mut cursor)?;

    let base_state = fid_states
        .get(&fid)
        .cloned()
        .unwrap_or_else(|| FidState::new("/".to_string(), encode_qid("/")));
    let mut current_path = base_state.path.clone();
    let mut qids: Vec<[u8; 13]> = Vec::new();

    for _ in 0..nwname {
        let name = read_string(&mut cursor)?;
        match resolve_step(&current_path, &name) {
            Some(next_path) => {
                if path_exists(&next_path, ramfs) {
                    current_path = next_path.clone();
                    qids.push(encode_qid(&next_path));
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

    let new_qid = qids.last().copied().unwrap_or(base_state.qid);
    fid_states.insert(newfid, FidState::new(current_path.clone(), new_qid));

    let mut response = Vec::new();
    response.extend_from_slice(&(qids.len() as u16).to_le_bytes());
    for qid in qids {
        response.extend_from_slice(&qid);
    }
    send_response(stream, RWALK, tag, &response)
}

fn handle_open(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &mut HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let _mode = read_u8(&mut cursor)?;

    let path = match fid_states.get(&fid) {
        Some(state) => state.path.clone(),
        None => return send_error(stream, tag, "unknown fid"),
    };

    if !path_exists(path.as_str(), ramfs) {
        return send_error(stream, tag, "file not found");
    }

    let state = fid_states.get_mut(&fid).unwrap();
    state.qid = encode_qid(path.as_str());
    state.open_mode = Some(_mode);

    let mut response = Vec::new();
    response.extend_from_slice(&state.qid);
    response.extend_from_slice(&0u32.to_le_bytes());
    send_response(stream, ROPEN, tag, &response)
}

fn handle_read(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let offset = read_u64(&mut cursor)?;
    let count = read_u32(&mut cursor)?;

    let state = match fid_states.get(&fid) {
        Some(state) => state,
        None => return send_error(stream, tag, "unknown fid"),
    };

    let mode = match state.open_mode {
        Some(mode) => mode,
        None => return send_error(stream, tag, "fid not open"),
    };

    if !mode_allows_read(mode) {
        return send_error(stream, tag, "fid not open for read");
    }

    let path = state.path.clone();
    let data = {
        let guard = ramfs.lock().unwrap();
        if let Some(bytes) = guard.read_file(&path) {
            Some(bytes.to_vec())
        } else if let Some(entries) = guard.list_dir(&path) {
            let joined = entries.join("\n") + "\n";
            Some(joined.into_bytes())
        } else {
            None
        }
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
    fid_states: &HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let offset = read_u64(&mut cursor)?;
    let count = read_u32(&mut cursor)?;
    let mut buffer = vec![0u8; count as usize];
    cursor.read_exact(&mut buffer)?;

    let state = match fid_states.get(&fid) {
        Some(state) => state,
        None => return send_error(stream, tag, "unknown fid"),
    };

    let mode = match state.open_mode {
        Some(mode) => mode,
        None => return send_error(stream, tag, "fid not open"),
    };

    if !mode_allows_write(mode) {
        return send_error(stream, tag, "fid not open for write");
    }

    let path = state.path.clone();
    let written = {
        let mut guard = ramfs.lock().unwrap();
        guard.write(&path, offset, &buffer).unwrap_or(0)
    };

    let mut response = Vec::new();
    response.extend_from_slice(&written.to_le_bytes());
    send_response(stream, RWRITE, tag, &response)
}

fn handle_wstat(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    let path = match fid_states.get(&fid) {
        Some(state) => state.path.clone(),
        None => return send_error(stream, tag, "unknown fid"),
    };

    let stat_to_set = match planten_9p::decode_stat(&mut cursor) {
        Ok(s) => s,
        Err(e) => return send_error(stream, tag, &format!("invalid stat format: {}", e)),
    };

    let mut guard = ramfs.lock().unwrap();

    if guard.wstat_from_stat(&path, &stat_to_set).is_some() {
        send_response(stream, RWSTAT, tag, &[])
    } else {
        send_error(stream, tag, "wstat failed")
    }
}

fn handle_flush(stream: &mut TcpStream, tag: u16, _body: &[u8]) -> io::Result<()> {
    send_response(stream, RFLUSH, tag, &[])
}

fn handle_remove(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &mut HashMap<u32, FidState>,
    ramfs: &Arc<Mutex<RamFs>>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;

    let path = match fid_states.get(&fid) {
        Some(state) => state.path.clone(),
        None => return send_error(stream, tag, "unknown fid"),
    };

    let success = {
        let mut guard = ramfs.lock().unwrap();
        guard.remove(&path).is_some()
    };

    fid_states.remove(&fid);

    if success {
        send_response(stream, RREMOVE, tag, &[])
    } else {
        send_error(stream, tag, "remove failed")
    }
}

fn handle_clunk(
    stream: &mut TcpStream,
    tag: u16,
    body: &[u8],
    fid_states: &mut HashMap<u32, FidState>,
) -> io::Result<()> {
    let mut cursor = Cursor::new(body);
    let fid = read_u32(&mut cursor)?;
    fid_states.remove(&fid);
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

fn build_stat(
    name: &str,
    length: u64,
    mode: u32,
    uid: &str,
    gid: &str,
    atime: u32,
    mtime: u32,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let qid = encode_qid(name);
    let stat = vec![
        0u16.to_le_bytes().to_vec(), // type
        0u32.to_le_bytes().to_vec(), // dev
        qid.to_vec(),
        mode.to_le_bytes().to_vec(),
        atime.to_le_bytes().to_vec(),
        mtime.to_le_bytes().to_vec(),
        length.to_le_bytes().to_vec(),
        encode_string_as_bytes(name),
        encode_string_as_bytes(uid),
        encode_string_as_bytes(gid),
        encode_string_as_bytes(uid), // muid
    ];
    let stat_bytes: Vec<u8> = stat.into_iter().flatten().collect();
    buf.extend_from_slice(&(stat_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&stat_bytes);
    buf
}

fn encode_string_as_bytes(s: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(s.len() as u16).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf
}

fn mode_allows_read(mode: u8) -> bool {
    matches!(mode & 0x3, 0 | 2 | 3)
}

fn mode_allows_write(mode: u8) -> bool {
    let typ = mode & 0x3;
    typ == 1 || typ == 2 || (mode & 0x10 != 0)
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
