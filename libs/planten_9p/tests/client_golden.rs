use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use planten_9p::RawMessage;
use planten_9p::messages::{self, TCLONE, TFLUSH, TREAD, TREMOVE, TSTAT, TVERSION, TWALK, TWSTAT};

const SESSION_TRACE: &str = "libs/planten_9p/tests/golden_traces/client_session.bin";

fn parse_frames(path: &str) -> Vec<(Vec<u8>, RawMessage)> {
    let bytes = fs::read(path).unwrap();
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let size = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let chunk = bytes[pos..pos + size].to_vec();
        let raw = RawMessage::from_bytes(&chunk).unwrap();
        frames.push((chunk, raw));
        pos += size;
    }
    frames
}

fn is_request(msg: u8) -> bool {
    matches!(
        msg,
        messages::TVERSION
            | messages::TATTACH
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
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
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
    let frames = parse_frames(SESSION_TRACE);
    let (listener, handle) = spawn_fake_server(frames.clone());
    let addr = listener.local_addr().unwrap();
    run_trace_session(&addr.to_string(), &frames, true).unwrap();
    handle.join().unwrap().unwrap();
}
