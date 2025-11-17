use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::{
    build_frame, encode_attach_body, encode_open_body, encode_read_body, encode_version_body,
    encode_walk_body, messages::*, RawMessage,
};
use planten_fs_net::{server, NetFs};

struct TraceRecorder {
    stream: TcpStream,
    next_tag: u16,
}

impl TraceRecorder {
    fn new(stream: TcpStream) -> Self {
        TraceRecorder {
            stream,
            next_tag: 0,
        }
    }

    fn send(&mut self, msg_type: u8, body: Vec<u8>) -> io::Result<RawMessage> {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1);
        let frame = build_frame(msg_type, tag, &body);
        self.stream.write_all(&frame)?;
        RawMessage::read_from(&mut self.stream)
    }
}

struct Operation {
    msg_type: u8,
    body: Vec<u8>,
    expected_response: String,
}

impl Operation {
    fn new(msg_type: u8, body: Vec<u8>, expected_response: String) -> Self {
        Operation {
            msg_type,
            body,
            expected_response,
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("expected libs directory")
        .parent()
        .expect("expected repo root")
        .to_path_buf()
}

fn expected_msg_types(traces_dir: &PathBuf) -> io::Result<HashMap<String, u8>> {
    let mut map = HashMap::new();
    for entry in fs::read_dir(traces_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap()
            .to_string();
        let raw = RawMessage::from_bytes(&fs::read(&path)?)?;
        map.insert(name, raw.msg_type);
    }
    Ok(map)
}

fn operations() -> Vec<Operation> {
    let mut ops = vec![
        Operation::new(
            TVERSION,
            encode_version_body(131072, "9P2000"),
            "rversion_response.bin".to_string(),
        ),
        Operation::new(
            TATTACH,
            encode_attach_body(1, None, "user", ""),
            "rattach_response.bin".to_string(),
        ),
        Operation::new(
            TOPEN,
            encode_open_body(1, 0),
            "ropen_root_response.bin".to_string(),
        ),
        Operation::new(
            TREAD,
            encode_read_body(1, 0, 4096),
            "rread_root_response.bin".to_string(),
        ),
    ];

    for (idx, entry) in NetFs::entries().iter().enumerate() {
        let fid = 2 + idx as u32;
        ops.push(Operation::new(
            TWALK,
            encode_walk_body(1, fid, &[entry]),
            format!("rwalk_{}_response.bin", entry),
        ));
        ops.push(Operation::new(
            TOPEN,
            encode_open_body(fid, 0),
            format!("ropen_{}_response.bin", entry),
        ));
        ops.push(Operation::new(
            TREAD,
            encode_read_body(fid, 0, 4096),
            format!("rread_{}_response.bin", entry),
        ));
    }

    ops
}

#[test]
fn net_golden_trace_matches_server() {
    let traces_dir = repo_root().join("tests/net_golden");
    let expected_types = expected_msg_types(&traces_dir).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let netfs = Arc::new(Mutex::new(NetFs));
    let server_netfs = Arc::clone(&netfs);
    thread::spawn(move || server::run_single(listener, server_netfs).unwrap());

    let mut recorder = TraceRecorder::new(TcpStream::connect(addr).unwrap());
    for op in operations() {
        let actual = recorder.send(op.msg_type, op.body).unwrap();
        let expected_type = expected_types.get(&op.expected_response).unwrap();
        assert_eq!(actual.msg_type, *expected_type);
    }
}
