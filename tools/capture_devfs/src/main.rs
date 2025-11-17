use std::fs;
use std::io;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::{
    build_frame, encode_attach_body, encode_open_body, encode_read_body, encode_version_body,
    encode_walk_body, messages::*, RawMessage,
};
use planten_fs_dev::{server, DevFs};

struct TraceRecorder<'a> {
    stream: &'a mut TcpStream,
    next_tag: u16,
}

impl<'a> TraceRecorder<'a> {
    fn new(stream: &'a mut TcpStream) -> Self {
        TraceRecorder {
            stream,
            next_tag: 0,
        }
    }

    fn next_tag(&mut self) -> u16 {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1);
        tag
    }

    fn send(&mut self, msg_type: u8, body: Vec<u8>) -> io::Result<(Vec<u8>, RawMessage)> {
        let tag = self.next_tag();
        let frame = build_frame(msg_type, tag, &body);
        self.stream.write_all(&frame)?;
        let response = RawMessage::read_from(&mut *self.stream)?;
        Ok((frame, response))
    }
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let devfs = Arc::new(Mutex::new(DevFs));
    let server_devfs = Arc::clone(&devfs);
    thread::spawn(move || {
        server::run_single(listener, server_devfs).unwrap();
    });

    let repo_root = repo_root();
    let traces_dir = repo_root.join("tests/dev_golden");
    fs::create_dir_all(&traces_dir)?;

    let mut stream = TcpStream::connect(addr)?;
    let mut recorder = TraceRecorder::new(&mut stream);

    capture_pair(
        &mut recorder,
        &traces_dir,
        "tversion_request.bin",
        "rversion_response.bin",
        TVERSION,
        encode_version_body(131072, "9P2000"),
    )?;
    capture_pair(
        &mut recorder,
        &traces_dir,
        "tattach_request.bin",
        "rattach_response.bin",
        TATTACH,
        encode_attach_body(1, None, "user", ""),
    )?;

    capture_pair(
        &mut recorder,
        &traces_dir,
        "topen_root_request.bin",
        "ropen_root_response.bin",
        TOPEN,
        encode_open_body(1, 0),
    )?;
    capture_pair(
        &mut recorder,
        &traces_dir,
        "tread_root_request.bin",
        "rread_root_response.bin",
        TREAD,
        encode_read_body(1, 0, 4096),
    )?;

    for (idx, entry) in DevFs::entries().iter().enumerate() {
        let fid = 2u32 + idx as u32;
        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("twalk_{}_request.bin", entry),
            &format!("rwalk_{}_response.bin", entry),
            TWALK,
            encode_walk_body(1, fid, &[entry]),
        )?;
        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("topen_{}_request.bin", entry),
            &format!("ropen_{}_response.bin", entry),
            TOPEN,
            encode_open_body(fid, 0),
        )?;
        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("tread_{}_request.bin", entry),
            &format!("rread_{}_response.bin", entry),
            TREAD,
            encode_read_body(fid, 0, 256),
        )?;
    }

    Ok(())
}

fn capture_pair(
    recorder: &mut TraceRecorder<'_>,
    traces_dir: &Path,
    request_name: &str,
    response_name: &str,
    msg_type: u8,
    body: Vec<u8>,
) -> io::Result<()> {
    let (request, response) = recorder.send(msg_type, body)?;
    write_file(traces_dir.join(request_name), &request)?;
    write_file(traces_dir.join(response_name), &frame_buf(&response))?;
    Ok(())
}

fn frame_buf(response: &RawMessage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(response.body.len() + 7);
    buf.extend_from_slice(&response.size.to_le_bytes());
    buf.push(response.msg_type);
    buf.extend_from_slice(&response.tag.to_le_bytes());
    buf.extend_from_slice(&response.body);
    buf
}

fn write_file(path: PathBuf, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("expected tools dir")
        .parent()
        .expect("expected repo root")
        .to_path_buf()
}
