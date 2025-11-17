use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use planten_9p::{
    RawMessage, Stat, build_frame, decode_stat, encode_attach_body, encode_clone_body,
    encode_create_body, encode_open_body, encode_read_body, encode_remove_body, encode_stat_body,
    encode_version_body, encode_walk_body, encode_write_body, encode_wstat_body, messages::*,
};
use planten_fs_ramfs::{RamFs, server};

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

    fn write(&mut self, fid: u32, offset: u64, data: &[u8]) -> io::Result<RawMessage> {
        self.send(TWRITE, encode_write_body(fid, offset, data))
    }

    fn clone_fid(&mut self, fid: u32, newfid: u32) -> io::Result<RawMessage> {
        self.send(TCLONE, encode_clone_body(fid, newfid))
    }

    fn stat(&mut self, fid: u32) -> io::Result<RawMessage> {
        self.send(TSTAT, encode_stat_body(fid))
    }

    fn wstat(&mut self, fid: u32, stat: &Stat) -> io::Result<RawMessage> {
        self.send(TWSTAT, encode_wstat_body(fid, stat))
    }

    fn create(&mut self, fid: u32, name: &str, perm: u32, mode: u8) -> io::Result<RawMessage> {
        self.send(TCREATE, encode_create_body(fid, name, perm, mode))
    }

    fn remove(&mut self, fid: u32) -> io::Result<RawMessage> {
        self.send(TREMOVE, encode_remove_body(fid))
    }
}

fn setup_ramfs_server() -> (TcpListener, Arc<Mutex<RamFs>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let ramfs = Arc::new(Mutex::new({
        let mut base = RamFs::new();
        base.create_file("/hello.txt", b"hello 9p!!");
        base
    }));
    (listener, ramfs)
}

fn decode_error_message(body: &[u8]) -> String {
    let mut cursor = Cursor::new(body);
    let len = read_u16(&mut cursor).unwrap() as usize;
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

fn read_u16(cursor: &mut Cursor<&[u8]>) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

#[test]
fn read_requires_open_mode() {
    let (listener, ramfs) = setup_ramfs_server();
    let addr = listener.local_addr().unwrap();
    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || server::run_single(listener, server_ramfs).unwrap());

    let mut session = TestSession::connect(&addr.to_string()).unwrap();
    session.handshake().unwrap();

    let walk_response = session.walk(1, 2, &["hello.txt"]).unwrap();
    assert_eq!(walk_response.msg_type, RWALK);

    let read_response = session.read(2, 0, 8).unwrap();
    assert_eq!(read_response.msg_type, RERROR);
    assert!(
        decode_error_message(&read_response.body).contains("fid not open"),
        "unexpected error body"
    );

    drop(session);
    server_thread.join().unwrap();
}

#[test]
fn write_requires_write_mode() {
    let (listener, ramfs) = setup_ramfs_server();
    let addr = listener.local_addr().unwrap();
    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || server::run_single(listener, server_ramfs).unwrap());

    let mut session = TestSession::connect(&addr.to_string()).unwrap();
    session.handshake().unwrap();
    session.walk(1, 2, &["hello.txt"]).unwrap();
    let open_response = session.open(2, 0).unwrap(); // read-only
    assert_eq!(open_response.msg_type, ROPEN);

    let write_response = session.write(2, 0, b"noop").unwrap();
    assert_eq!(write_response.msg_type, RERROR);
    assert!(
        decode_error_message(&write_response.body).contains("fid not open for write"),
        "unexpected error body"
    );

    drop(session);
    server_thread.join().unwrap();
}

#[test]
fn clone_preserves_open_mode() {
    let (listener, ramfs) = setup_ramfs_server();
    let addr = listener.local_addr().unwrap();
    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || server::run_single(listener, server_ramfs).unwrap());

    let mut session = TestSession::connect(&addr.to_string()).unwrap();
    session.handshake().unwrap();
    session.walk(1, 2, &["hello.txt"]).unwrap();
    let open_response = session.open(2, 0).unwrap(); // read-only
    assert_eq!(open_response.msg_type, ROPEN);

    let clone_response = session.clone_fid(2, 3).unwrap();
    assert_eq!(clone_response.msg_type, RCLONE);

    let read_response = session.read(3, 0, 4).unwrap();
    assert_eq!(read_response.msg_type, RREAD);
    assert!(!read_response.body.is_empty());

    drop(session);
    server_thread.join().unwrap();
}

#[test]
fn stat_returns_file_info() {
    let (listener, ramfs) = setup_ramfs_server();
    let addr = listener.local_addr().unwrap();
    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || server::run_single(listener, server_ramfs).unwrap());

    let mut session = TestSession::connect(&addr.to_string()).unwrap();
    session.handshake().unwrap();

    let walk_response = session.walk(1, 2, &["hello.txt"]).unwrap();
    assert_eq!(walk_response.msg_type, RWALK);

    let stat_response = session.stat(2).unwrap();
    assert_eq!(stat_response.msg_type, RSTAT);

    let mut cursor = Cursor::new(stat_response.body.as_slice());
    let stat = decode_stat(&mut cursor).unwrap();
    assert_eq!(stat.name, "hello.txt");
    assert_eq!(stat.length, 10); // "hello 9p!!" is 10 bytes
    assert_eq!(stat.mode & 0o777, 0o644);

    drop(session);
    server_thread.join().unwrap();
}

#[test]
fn wstat_modifies_file_info() {
    let (listener, ramfs) = setup_ramfs_server();
    let addr = listener.local_addr().unwrap();
    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || server::run_single(listener, server_ramfs).unwrap());

    let mut session = TestSession::connect(&addr.to_string()).unwrap();
    session.handshake().unwrap();

    let walk_response = session.walk(1, 2, &["hello.txt"]).unwrap();
    assert_eq!(walk_response.msg_type, RWALK);

    // Get initial stat
    let stat_response = session.stat(2).unwrap();
    let mut cursor = Cursor::new(stat_response.body.as_slice());
    let original_stat = decode_stat(&mut cursor).unwrap();

    // Send wstat to change mode and name
    let new_stat = Stat {
        name: "new_hello.txt".to_string(),
        mode: 0o777,
        ..original_stat
    };
    let wstat_response = session.wstat(2, &new_stat).unwrap();
    assert_eq!(wstat_response.msg_type, RWSTAT);

    // Stat again to check changes
    // We need to walk again because the fid is for the old path.
    // A better test would re-use the fid if the name is not changed.
    let walk_response = session.walk(1, 3, &["new_hello.txt"]).unwrap();
    assert_eq!(walk_response.msg_type, RWALK);

    let stat_response = session.stat(3).unwrap();
    assert_eq!(stat_response.msg_type, RSTAT);

    let mut cursor = Cursor::new(stat_response.body.as_slice());
    let updated_stat = decode_stat(&mut cursor).unwrap();
    assert_eq!(updated_stat.name, "new_hello.txt");
    assert_eq!(updated_stat.mode & 0o777, 0o777);

    drop(session);
    server_thread.join().unwrap();
}

#[test]
fn create_and_remove_file() {
    let (listener, ramfs) = setup_ramfs_server();
    let addr = listener.local_addr().unwrap();
    let server_ramfs = Arc::clone(&ramfs);
    let server_thread = thread::spawn(move || server::run_single(listener, server_ramfs).unwrap());

    let mut session = TestSession::connect(&addr.to_string()).unwrap();
    session.handshake().unwrap();

    // Create a new file
    let create_response = session.create(1, "new_file.txt", 0o644, 1).unwrap(); // fid 1 is root
    assert_eq!(create_response.msg_type, RCREATE);

    // Walk to the new file to verify it exists
    let walk_response = session.walk(1, 2, &["new_file.txt"]).unwrap();
    assert_eq!(walk_response.msg_type, RWALK);

    // Remove the file
    let remove_response = session.remove(2).unwrap();
    assert_eq!(remove_response.msg_type, RREMOVE);

    // Walk again to verify it's gone
    let walk_response = session.walk(1, 3, &["new_file.txt"]).unwrap();
    assert_eq!(walk_response.msg_type, RERROR);

    drop(session);
    server_thread.join().unwrap();
}
