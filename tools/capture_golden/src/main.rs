use std::fs::{self, File};
use std::io::{self, Cursor, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::{
    build_frame, decode_stat, encode_attach_body, encode_auth_body, encode_clone_body,
    encode_flush_body, encode_open_body, encode_read_body, encode_remove_body, encode_stat_body,
    encode_version_body, encode_walk_body, encode_write_body, encode_wstat_body, messages::*,
    RawMessage,
};
use planten_fs_ramfs::{server, RamFs};

struct TraceRecorder<'a> {
    stream: &'a mut TcpStream,
    next_tag: u16,
}

impl<'a> TraceRecorder<'a> {
    fn new(stream: &'a mut TcpStream) -> Self {
        Self {
            stream,
            next_tag: 0,
        }
    }

    fn send(&mut self, msg_type: u8, body: Vec<u8>) -> io::Result<(Vec<u8>, RawMessage)> {
        let tag = self.next_tag();
        let frame = build_frame(msg_type, tag, &body);
        self.stream.write_all(&frame)?;
        let response = RawMessage::read_from(&mut *self.stream)?;
        Ok((frame, response))
    }

    fn send_with_tag(
        &mut self,
        msg_type: u8,
        body: Vec<u8>,
        tag: u16,
    ) -> io::Result<(Vec<u8>, RawMessage)> {
        let frame = build_frame(msg_type, tag, &body);
        self.stream.write_all(&frame)?;
        let response = RawMessage::read_from(&mut *self.stream)?;
        Ok((frame, response))
    }

    fn next_tag(&mut self) -> u16 {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1);
        tag
    }
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    let ramfs = Arc::new(Mutex::new({
        let mut base = RamFs::new();
        base.create_file("/hello.txt", b"hello 9p!!");
        base.create_file("/readme.txt", b"RAMFS as a 9P server");
        base
    }));

    let server_ramfs = Arc::clone(&ramfs);
    thread::spawn(move || server::run_server(listener, server_ramfs).unwrap());

    let repo_root = repo_root();
    let traces_dir = repo_root.join("tests/golden_traces");
    fs::create_dir_all(&traces_dir)?;

    let mut stream = TcpStream::connect(addr)?;
    let mut recorder = TraceRecorder::new(&mut stream);

    let handshake_frames = capture_handshake(&mut recorder)?;
    write_frames(traces_dir.join("handshake.bin"), &handshake_frames)?;

    let (walk_req, walk_resp, _) =
        capture_exchange(&mut recorder, TWALK, encode_walk_body(1, 2, &["hello.txt"]))?;
    write_file(traces_dir.join("twalk_request.bin"), &walk_req)?;
    write_file(traces_dir.join("walk_response.bin"), &walk_resp)?;

    // Open fid 2 for subsequent read/write operations.
    let (data_open_req, data_open_resp, _) =
        capture_exchange(&mut recorder, TOPEN, encode_open_body(2, 2))?;
    write_file(traces_dir.join("topen_data_request.bin"), &data_open_req)?;
    write_file(traces_dir.join("ropen_data_response.bin"), &data_open_resp)?;

    let (read_req, read_resp, _) =
        capture_exchange(&mut recorder, TREAD, encode_read_body(2, 0, 128))?;
    let read_exchange_frames = vec![read_req.clone(), read_resp.clone()];
    write_frames(traces_dir.join("read_exchange.bin"), &read_exchange_frames)?;
    write_file(traces_dir.join("read_response.bin"), &read_resp)?;

    let (root_open_req, root_open_resp, _) =
        capture_exchange(&mut recorder, TOPEN, encode_open_body(1, 0))?;
    write_file(traces_dir.join("topen_root_request.bin"), &root_open_req)?;
    write_file(traces_dir.join("ropen_root_response.bin"), &root_open_resp)?;

    let (dir_req, dir_resp, _) =
        capture_exchange(&mut recorder, TREAD, encode_read_body(1, 0, 128))?;
    write_file(traces_dir.join("tread_dir_request.bin"), &dir_req)?;
    write_file(traces_dir.join("dir_read_response.bin"), &dir_resp)?;

    let (stat_req, stat_resp, stat_msg) =
        capture_exchange(&mut recorder, TSTAT, encode_stat_body(2))?;
    write_file(traces_dir.join("tstat_request.bin"), &stat_req)?;
    write_file(traces_dir.join("rstat_response.bin"), &stat_resp)?;
    let mut stat = decode_stat(&mut Cursor::new(stat_msg.body.as_slice()))?;

    let content = b"hello world";
    let (write_req, write_resp, _) =
        capture_exchange(&mut recorder, TWRITE, encode_write_body(2, 0, content))?;
    let write_exchange_frames = vec![write_req.clone(), write_resp.clone()];
    write_frames(
        traces_dir.join("write_exchange.bin"),
        &write_exchange_frames,
    )?;
    write_file(traces_dir.join("rwrite_response.bin"), &write_resp)?;

    stat.length = content.len() as u64;
    let (twstat_req, twstat_resp, _) =
        capture_exchange(&mut recorder, TWSTAT, encode_wstat_body(2, &stat))?;
    write_file(traces_dir.join("twstat_request.bin"), &twstat_req)?;
    write_file(traces_dir.join("rwstat_response.bin"), &twstat_resp)?;

    let (remove_req, remove_resp, _) =
        capture_exchange(&mut recorder, TREMOVE, encode_remove_body(2))?;
    let remove_exchange_frames = vec![remove_req.clone(), remove_resp.clone()];
    write_frames(
        traces_dir.join("remove_exchange.bin"),
        &remove_exchange_frames,
    )?;
    write_file(traces_dir.join("rremove_response.bin"), &remove_resp)?;

    let (walk_err_req, walk_err_resp, _) = capture_exchange(
        &mut recorder,
        TWALK,
        encode_walk_body(1, 3, &["missing.txt"]),
    )?;
    write_file(traces_dir.join("twalk_error_request.bin"), &walk_err_req)?;
    write_file(traces_dir.join("rerror_walk.bin"), &walk_err_resp)?;

    let (walk_multi_req, walk_multi_resp, _) = capture_exchange(
        &mut recorder,
        TWALK,
        encode_walk_body(1, 4, &["hello.txt", "missing.txt"]),
    )?;
    write_file(traces_dir.join("twalk_multi_request.bin"), &walk_multi_req)?;
    write_file(traces_dir.join("rerror_walk_multi.bin"), &walk_multi_resp)?;

    let (flush_req, flush_resp, _) = capture_exchange(&mut recorder, TFLUSH, encode_flush_body(1))?;
    write_file(traces_dir.join("tflush_request.bin"), &flush_req)?;
    write_file(traces_dir.join("rflush_response.bin"), &flush_resp)?;

    let (auth_req, auth_resp, _) =
        capture_exchange(&mut recorder, TAUTH, encode_auth_body(0, "user", ""))?;
    write_file(traces_dir.join("tauth_request.bin"), &auth_req)?;
    write_file(traces_dir.join("rauth_response.bin"), &auth_resp)?;

    let (clone_req, clone_resp, _) =
        capture_exchange_with_tag(&mut recorder, TCLONE, encode_clone_body(1, 5), 0x9999)?;
    write_file(traces_dir.join("tclone_request.bin"), &clone_req)?;
    write_file(traces_dir.join("rclone_response.bin"), &clone_resp)?;

    let (tstat_err_req, tstat_err_resp, _) =
        capture_exchange(&mut recorder, TSTAT, encode_stat_body(99))?;
    write_file(traces_dir.join("tstat_error_request.bin"), &tstat_err_req)?;
    write_file(traces_dir.join("rerror_tstat.bin"), &tstat_err_resp)?;

    let (read_oob_req, read_oob_resp, _) =
        capture_exchange(&mut recorder, TREAD, encode_read_body(99, 0, 1))?;
    write_file(traces_dir.join("tread_oob_request.bin"), &read_oob_req)?;
    write_file(traces_dir.join("rerror_oob.bin"), &read_oob_resp)?;

    Ok(())
}

fn capture_handshake(recorder: &mut TraceRecorder<'_>) -> io::Result<Vec<Vec<u8>>> {
    let mut frames = Vec::new();
    let (version_req, version_resp) =
        recorder.send(TVERSION, encode_version_body(131072, "9P2000"))?;
    frames.push(version_req);
    frames.push(frame_buf(&version_resp));

    let (attach_req, attach_resp) =
        recorder.send(TATTACH, encode_attach_body(1, None, "user", ""))?;
    frames.push(attach_req);
    frames.push(frame_buf(&attach_resp));

    Ok(frames)
}

fn write_frames(path: impl AsRef<Path>, frames: &[Vec<u8>]) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    for frame in frames {
        file.write_all(frame)?;
    }
    Ok(())
}

fn write_file(path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn frame_buf(message: &RawMessage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(message.size as usize);
    buf.extend_from_slice(&message.size.to_le_bytes());
    buf.push(message.msg_type);
    buf.extend_from_slice(&message.tag.to_le_bytes());
    buf.extend_from_slice(&message.body);
    buf
}

fn capture_exchange(
    recorder: &mut TraceRecorder<'_>,
    msg_type: u8,
    body: Vec<u8>,
) -> io::Result<(Vec<u8>, Vec<u8>, RawMessage)> {
    let (req, resp) = recorder.send(msg_type, body)?;
    let resp_bytes = frame_buf(&resp);
    Ok((req, resp_bytes, resp))
}

fn capture_exchange_with_tag(
    recorder: &mut TraceRecorder<'_>,
    msg_type: u8,
    body: Vec<u8>,
    tag: u16,
) -> io::Result<(Vec<u8>, Vec<u8>, RawMessage)> {
    let (req, resp) = recorder.send_with_tag(msg_type, body, tag)?;
    let resp_bytes = frame_buf(&resp);
    Ok((req, resp_bytes, resp))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("expected tools dir")
        .parent()
        .expect("expected repo root")
        .to_path_buf()
}
