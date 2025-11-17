use std::env;
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
use planten_fs_srv::{server, SrvFs};
use tempfile::tempdir;

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
    let temp = tempdir()?;
    let services = ["planten", "service"];
    for service in &services {
        let path = temp.path().join(service);
        fs::create_dir_all(&path)?;
        fs::write(path.join("ctl"), format!("service {} ctl\n", service))?;
    }
    env::set_var("PLANTEN_SRV_ROOT", temp.path());

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let srvfs = Arc::new(Mutex::new(SrvFs::new()));
    let server_srvfs = Arc::clone(&srvfs);
    thread::spawn(move || {
        server::run_single(listener, server_srvfs).unwrap();
    });

    let repo_root = repo_root();
    let traces_dir = repo_root.join("tests/srv_golden");
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

    for (idx, service) in services.iter().enumerate() {
        let service_fid = 2 + (idx as u32 * 2);
        let ctl_fid = service_fid + 1;

        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("twalk_{}_request.bin", service),
            &format!("rwalk_{}_response.bin", service),
            TWALK,
            encode_walk_body(1, service_fid, &[service]),
        )?;
        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("topen_{}_request.bin", service),
            &format!("ropen_{}_response.bin", service),
            TOPEN,
            encode_open_body(service_fid, 0),
        )?;
        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("tread_{}_request.bin", service),
            &format!("rread_{}_response.bin", service),
            TREAD,
            encode_read_body(service_fid, 0, 4096),
        )?;

        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("twalk_{}_ctl_request.bin", service),
            &format!("rwalk_{}_ctl_response.bin", service),
            TWALK,
            encode_walk_body(service_fid, ctl_fid, &["ctl"]),
        )?;
        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("topen_{}_ctl_request.bin", service),
            &format!("ropen_{}_ctl_response.bin", service),
            TOPEN,
            encode_open_body(ctl_fid, 0),
        )?;
        capture_pair(
            &mut recorder,
            &traces_dir,
            &format!("tread_{}_ctl_request.bin", service),
            &format!("rread_{}_ctl_response.bin", service),
            TREAD,
            encode_read_body(ctl_fid, 0, 4096),
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
