use planten_fs_core::{FsServer, Inode};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const NET_ENTRIES: &[&str] = &["interfaces", "tcp", "udp"];

pub struct NetFs;

impl NetFs {
    fn list_entries() -> Vec<String> {
        NET_ENTRIES.iter().map(|v| v.to_string()).collect()
    }

    fn read_interfaces() -> io::Result<Vec<u8>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir("/sys/class/net")? {
            let name = entry?.file_name().into_string().unwrap_or_default();
            entries.push(name);
        }
        entries.sort();
        let mut buf = entries.join("\n");
        buf.push('\n');
        Ok(buf.into_bytes())
    }

    fn read_proc_file(file: &str) -> io::Result<Vec<u8>> {
        let path = Path::new("/proc/net").join(file);
        fs::read(path)
    }

    fn read_entry(name: &str) -> io::Result<Vec<u8>> {
        match name {
            "interfaces" => Self::read_interfaces(),
            "tcp" | "udp" => Self::read_proc_file(name),
            _ => Err(io::Error::new(io::ErrorKind::NotFound, "entry not found")),
        }
    }
}

fn make_inode(name: &str, data: &[u8]) -> Inode {
    let mut inode = Inode::new(name, 0o444, "root", "root");
    inode.data = data.to_vec();
    inode
}

impl FsServer for NetFs {
    fn walk(&self, path: &str) -> Option<Vec<String>> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => Some(Self::list_entries()),
            [name] if NET_ENTRIES.contains(name) => Some(vec![]),
            _ => None,
        }
    }

    fn open(&self, path: &str) -> Option<()> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => Some(()),
            [name] if NET_ENTRIES.contains(name) => Some(()),
            _ => None,
        }
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => {
                let entries = Self::list_entries();
                Some(entries.join("\n").to_string().into_bytes())
            }
            [name] if NET_ENTRIES.contains(name) => Self::read_entry(name).ok(),
            _ => None,
        }
    }

    fn write(&mut self, _path: &str, _offset: u64, _data: &[u8]) -> Option<u32> {
        None
    }

    fn clunk(&self, _path: &str) -> Option<()> {
        Some(())
    }

    fn remove(&mut self, _path: &str) -> Option<()> {
        None
    }

    fn stat(&self, path: &str) -> Option<Inode> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        match comps.as_slice() {
            [] => {
                let mut inode = Inode::new("net", 0o555 | 0x80000000, "root", "root");
                inode.atime = now;
                inode.mtime = now;
                Some(inode)
            }
            [name] if NET_ENTRIES.contains(name) => {
                if let Ok(data) = Self::read_entry(name) {
                    let mut inode = make_inode(name, &data);
                    inode.atime = now;
                    inode.mtime = now;
                    Some(inode)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn wstat(&mut self, _path: &str, _inode: Inode) -> Option<()> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::NetFs;

    #[test]
    fn list_entries() {
        let net = NetFs;
        let entries = NetFs::list_entries();
        assert!(entries.contains(&"tcp".to_string()));
        assert!(entries.contains(&"udp".to_string()));
    }
}
