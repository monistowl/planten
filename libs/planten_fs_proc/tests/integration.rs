use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::{
    build_frame, decode_u32, encode_attach_body, encode_open_body, encode_read_body,
    encode_version_body, encode_walk_body, messages::*, RawMessage,
};
use planten_fs_proc::fs::ProcFs;
use planten_fs_proc::server;

struct TestSession {
    stream: TcpStream,
    next_tag: u16,
}

impl TestSession {
    fn connect(addr: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self {
            stream,
            next_tag: 0,
        })
    }

    fn send(&mut self, msg_type: u8, body: Vec<u8>) -> io::Result<RawMessage> {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1);
        let frame = build_frame(msg_type, tag, &body);
        self.stream.write_all(&frame)?;
        RawMessage::read_from(&mut self.stream)
    }

    fn handshake(&mut self) -> io::Result<()> {
        let version = self.send(TVERSION, encode_version_body(131072, "9P2000"))?;
        assert_eq!(version.msg_type, RVERSION);
        let attach = self.send(TATTACH, encode_attach_body(1, None, "user", ""))?;
        assert_eq!(attach.msg_type, RATTACH);
        Ok(())
    }

    fn walk(&mut self, fid: u32, newfid: u32, names: &[&str]) -> io::Result<RawMessage> {
        self.send(TWALK, encode_walk_body(fid, newfid, names))
    }

    fn open(&mut self, fid: u32, mode: u8) -> io::Result<RawMessage> {
        self.send(TOPEN, encode_open_body(fid, mode))
    }

    fn read(&mut self, fid: u32, offset: u64, count: u32) -> io::Result<RawMessage> {
        self.send(TREAD, encode_read_body(fid, offset, count))
    }
}

fn setup_procfs_server() -> (TcpListener, Arc<Mutex<ProcFs>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let procfs = Arc::new(Mutex::new(ProcFs::new()));
    (listener, procfs)
}

#[test]
fn read_procfs() {
    let (listener, procfs) = setup_procfs_server();
    let addr = listener.local_addr().unwrap();
    let server_procfs = Arc::clone(&procfs);
    let server_thread =
        thread::spawn(move || server::run_single(listener, server_procfs).unwrap());

    let mut session = TestSession::connect(&addr.to_string()).unwrap();
    session.handshake().unwrap();

    // Read root to get PIDs
    let open_response = session.open(1, 0).unwrap(); // fid 1 is root
    assert_eq!(open_response.msg_type, ROPEN);
    let read_response = session.read(1, 0, 4096).unwrap();
    assert_eq!(read_response.msg_type, RREAD);

    let mut cursor = Cursor::new(read_response.body.as_slice());
    let len = decode_u32(&mut cursor).unwrap();
    let mut pids_bytes = vec![0; len as usize];
    cursor.read_exact(&mut pids_bytes).unwrap();
    let pids_str = String::from_utf8(pids_bytes).unwrap();
    let pids: Vec<&str> = pids_str.trim().split('\n').collect();
    let self_pid = std::process::id().to_string();
    assert!(pids.contains(&self_pid.as_str()));

    // Walk to self/status
    let walk_response = session.walk(1, 2, &[&self_pid, "status"]).unwrap();
    assert_eq!(walk_response.msg_type, RWALK);

    // Read self/status
    let open_response = session.open(2, 0).unwrap();
    assert_eq!(open_response.msg_type, ROPEN);
    let read_response = session.read(2, 0, 4096).unwrap();
    assert_eq!(read_response.msg_type, RREAD);

    let mut cursor = Cursor::new(read_response.body.as_slice());
    let len = decode_u32(&mut cursor).unwrap();
    let mut status_bytes = vec![0; len as usize];
    cursor.read_exact(&mut status_bytes).unwrap();
    let status_str = String::from_utf8(status_bytes).unwrap();
    assert!(status_str.contains(&format!("pid {}", self_pid)));

    drop(session);
    server_thread.join().unwrap();
}

