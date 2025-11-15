use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Inode {
    pub name: String,
    pub data: Vec<u8>,
    pub children: HashMap<String, Inode>,
    pub mode: u32, // Permissions and file type
    pub uid: String, // Owner user ID
    pub gid: String, // Group ID
    pub atime: u32, // Access time
    pub mtime: u32, // Modification time
}

impl Inode {
    pub fn new(name: &str, mode: u32, uid: &str, gid: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        Inode {
            name: name.to_string(),
            data: Vec::new(),
            children: HashMap::new(),
            mode,
            uid: uid.to_string(),
            gid: gid.to_string(),
            atime: now,
            mtime: now,
        }
    }
}

pub trait FsServer {
    fn walk(&self, path: &str) -> Option<Vec<String>>;
    fn open(&self, path: &str) -> Option<()>;
    fn read(&self, path: &str) -> Option<&[u8]>;
    fn write(&mut self, path: &str, offset: u64, data: &[u8]) -> Option<u32>;
    fn clunk(&self, path: &str) -> Option<()>;
    fn remove(&mut self, path: &str) -> Option<()>;
    fn stat(&self, path: &str) -> Option<Inode>;
    fn wstat(&mut self, path: &str, inode: Inode) -> Option<()>;
}
