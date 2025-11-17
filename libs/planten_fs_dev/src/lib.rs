use planten_fs_core::{FsServer, Inode};
use rand::random;
use std::time::{SystemTime, UNIX_EPOCH};

const DEV_ENTRIES: &[&str] = &["console", "null", "zero", "random"];

enum DevFile {
    Console,
    Null,
    Zero,
    Random,
}

impl DevFile {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "console" => Some(Self::Console),
            "null" => Some(Self::Null),
            "zero" => Some(Self::Zero),
            "random" => Some(Self::Random),
            _ => None,
        }
    }

    fn read(&self, len: usize) -> Vec<u8> {
        match self {
            DevFile::Console => b"console".to_vec(),
            DevFile::Null => Vec::new(),
            DevFile::Zero => vec![0u8; len.max(1)],
            DevFile::Random => (0..len.max(1)).map(|_| random::<u8>()).collect(),
        }
    }

    fn write(&self, len: usize) -> usize {
        match self {
            DevFile::Console => len,
            DevFile::Null => len,
            DevFile::Zero => len,
            DevFile::Random => len,
        }
    }
}

pub struct DevFs;

fn now() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32
}

impl FsServer for DevFs {
    fn walk(&self, path: &str) -> Option<Vec<String>> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => Some(DEV_ENTRIES.iter().map(|&s| s.to_string()).collect()),
            [name] if DEV_ENTRIES.contains(name) => Some(vec![]),
            _ => None,
        }
    }

    fn open(&self, path: &str) -> Option<()> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => Some(()),
            [name] if DEV_ENTRIES.contains(name) => Some(()),
            _ => None,
        }
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => {
                let entries = DEV_ENTRIES
                    .iter()
                    .map(|&s| s.to_string())
                    .collect::<Vec<_>>();
                Some(entries.join("\n").into_bytes())
            }
            [name] if DEV_ENTRIES.contains(name) => {
                let file = DevFile::from_name(name)?;
                Some(file.read(64))
            }
            _ => None,
        }
    }

    fn write(&mut self, path: &str, _offset: u64, data: &[u8]) -> Option<u32> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [name] if DEV_ENTRIES.contains(name) => {
                let file = DevFile::from_name(name)?;
                Some(file.write(data.len()) as u32)
            }
            _ => None,
        }
    }

    fn clunk(&self, _path: &str) -> Option<()> {
        Some(())
    }

    fn remove(&mut self, _path: &str) -> Option<()> {
        None
    }

    fn stat(&self, path: &str) -> Option<Inode> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => {
                let mut inode = Inode::new("dev", 0o555 | 0x80000000, "root", "root");
                inode.atime = now();
                inode.mtime = now();
                Some(inode)
            }
            [name] if DEV_ENTRIES.contains(name) => {
                let mut inode = Inode::new(name, 0o666, "root", "root");
                inode.atime = now();
                inode.mtime = now();
                Some(inode)
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
    use super::DevFs;
    use planten_fs_core::FsServer;

    #[test]
    fn root_has_null() {
        let dev = DevFs;
        let entries = dev.walk("").expect("root walk should succeed");
        assert!(entries.contains(&"null".to_string()));
    }
}
