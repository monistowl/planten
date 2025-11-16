use std::fs;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;

use planten_9p::RawMessage;
use planten_9p::messages::{self, TCLONE, TFLUSH, TREAD, TREMOVE, TSTAT, TWALK, TWSTAT};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf()
}

fn traces_dir() -> PathBuf {
    repo_root().join("tests").join("golden_traces")
}

fn read_trace(file: &str) -> Vec<u8> {
    fs::read(traces_dir().join(file)).expect("trace should exist")
}

fn parse_frames(bytes: &[u8]) -> Vec<(Vec<u8>, RawMessage)> {
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos + 4 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let chunk = bytes[pos..pos + size].to_vec();
        let raw = RawMessage::from_bytes(&chunk).unwrap();
        frames.push((chunk, raw));
        pos += size;
    }
    frames
}

fn load_frames(file: &str) -> Vec<(Vec<u8>, RawMessage)> {
    let bytes = read_trace(file);
    parse_frames(&bytes)
}

fn load_frame(file: &str) -> (Vec<u8>, RawMessage) {
    let bytes = read_trace(file);
    let raw = RawMessage::from_bytes(&bytes).unwrap();
    (bytes, raw)
}

fn push_pair(frames: &mut Vec<(Vec<u8>, RawMessage)>, request: &str, response: &str) {
    frames.push(load_frame(request));
    frames.push(load_frame(response));
}

fn session_frames() -> Vec<(Vec<u8>, RawMessage)> {
    let mut frames = Vec::new();
    frames.extend(load_frames("handshake.bin"));
    push_pair(&mut frames, "twalk_request.bin", "walk_response.bin");
    frames.extend(load_frames("read_exchange.bin"));
    push_pair(
        &mut frames,
        "topen_root_request.bin",
        "ropen_root_response.bin",
    );
    push_pair(&mut frames, "tread_dir_request.bin", "dir_read_response.bin");
    push_pair(&mut frames, "tstat_request.bin", "rstat_response.bin");
    frames.extend(load_frames("write_exchange.bin"));
    push_pair(&mut frames, "twstat_request.bin", "rwstat_response.bin");
    frames.extend(load_frames("remove_exchange.bin"));
    push_pair(&mut frames, "tflush_request.bin", "rflush_response.bin");
    push_pair(&mut frames, "tauth_request.bin", "rauth_response.bin");
    push_pair(&mut frames, "tclone_request.bin", "rclone_response.bin");
    push_pair(&mut frames, "tstat_error_request.bin", "rerror_tstat.bin");
    push_pair(&mut frames, "tread_oob_request.bin", "rerror_oob.bin");
    push_pair(&mut frames, "twalk_error_request.bin", "rerror_walk.bin");
    push_pair(
        &mut frames,
        "twalk_multi_request.bin",
        "rerror_walk_multi.bin",
    );
    frames
}

fn is_request(msg: u8) -> bool {
    matches!(
        msg,
        messages::TVERSION
            | messages::TATTACH
            | messages::TAUTH
            | TWALK
            | messages::TOPEN
            | TREAD
            | messages::TWRITE
            | TREMOVE
            | TCLONE
            | TSTAT
            | TWSTAT
            | TFLUSH
    )
}

fn run_trace_session(
    addr: &str,
    frames: &[(Vec<u8>, RawMessage)],
    compare_body: bool,
) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;
    for (chunk, frame) in frames {
        if is_request(frame.msg_type) {
            stream.write_all(chunk)?;
        } else {
            let response = RawMessage::read_from(&mut stream)?;
            assert_eq!(response.msg_type, frame.msg_type);
            if compare_body {
                assert_eq!(response.body, frame.body);
            }
        }
    }
    Ok(())
}

fn spawn_fake_server(
    frames: Vec<(Vec<u8>, RawMessage)>,
) -> (TcpListener, thread::JoinHandle<io::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server_listener = listener.try_clone().expect("clone listener");
    let handle = thread::spawn(move || {
        let (mut stream, _) = server_listener.accept()?;
        for (chunk, frame) in frames {
            if is_request(frame.msg_type) {
                let incoming = RawMessage::read_from(&mut stream)?;
                assert_eq!(incoming.msg_type, frame.msg_type);
                assert_eq!(incoming.body, frame.body);
            } else {
                stream.write_all(&chunk)?;
            }
        }
        Ok(())
    });
    (listener, handle)
}

#[test]
fn golden_client_session_parses() {
    let frames = session_frames();
    let (listener, handle) = spawn_fake_server(frames.clone());
    let addr = listener.local_addr().unwrap();
    run_trace_session(&addr.to_string(), &frames, true).unwrap();
    handle.join().unwrap().unwrap();
}
