use planten_fs_core::{FsServer, Inode};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_SRV_ROOT: &str = "/srv";
const SERVICE_FILES: &[&str] = &["ctl"];

fn srv_root() -> PathBuf {
    env::var("PLANTEN_SRV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_SRV_ROOT))
}

fn now() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32
}

fn read_service_ctls(root: &Path, name: &str) -> io::Result<Vec<u8>> {
    let path = root.join(name).join("ctl");
    if path.exists() {
        return fs::read(path);
    }
    Ok(format!("service {} ctl\n", name).into_bytes())
}

pub struct SrvFs;

impl SrvFs {
    pub fn new() -> Self {
        SrvFs
    }
}

impl SrvFs {
    fn list_services(&self) -> Vec<String> {
        let root = srv_root();
        match fs::read_dir(&root) {
            Ok(entries) => entries
                .filter_map(|entry| {
                    entry.ok().and_then(|e| {
                        let path = e.path();
                        if path.is_dir() {
                            path.file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    fn service_files() -> Vec<String> {
        SERVICE_FILES.iter().map(|s| s.to_string()).collect()
    }
}

impl FsServer for SrvFs {
    fn walk(&self, path: &str) -> Option<Vec<String>> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => Some(self.list_services()),
            [service] if !service.is_empty() => {
                if self.list_services().contains(&service.to_string()) {
                    Some(Self::service_files())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn open(&self, path: &str) -> Option<()> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => Some(()),
            [service] if self.list_services().contains(&service.to_string()) => Some(()),
            [service, file] if SERVICE_FILES.contains(file) => Some(()),
            _ => None,
        }
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match comps.as_slice() {
            [] => {
                let entries = self.list_services();
                Some(entries.join("\n").into_bytes())
            }
            [service] if self.list_services().contains(&service.to_string()) => {
                let files = Self::service_files();
                Some(files.join("\n").into_bytes())
            }
            [service, file] if SERVICE_FILES.contains(file) => {
                read_service_ctls(&srv_root(), service).ok()
            }
            _ => None,
        }
    }

    fn write(&mut self, path: &str, _offset: u64, data: &[u8]) -> Option<u32> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if let [service, file] = comps.as_slice() {
            if SERVICE_FILES.contains(file) && !service.is_empty() {
                return Some(data.len() as u32);
            }
        }
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
        let timestamp = now();
        match comps.as_slice() {
            [] => {
                let mut inode = Inode::new("srv", 0o555 | 0x80000000, "root", "root");
                inode.atime = timestamp;
                inode.mtime = timestamp;
                Some(inode)
            }
            [service] if self.list_services().contains(&service.to_string()) => {
                let mut inode = Inode::new(service, 0o555 | 0x80000000, "root", "root");
                inode.atime = timestamp;
                inode.mtime = timestamp;
                Some(inode)
            }
            [service, file] if SERVICE_FILES.contains(file) => {
                if let Some(data) = self.read(path) {
                    let mut inode = Inode::new(file, 0o444, "root", "root");
                    inode.data = data;
                    inode.atime = timestamp;
                    inode.mtime = timestamp;
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

pub mod server;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn list_services_dir() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("foo")).unwrap();
        env::set_var("PLANTEN_SRV_ROOT", temp.path());
        let srv = SrvFs;
        assert!(srv.list_services().contains(&"foo".to_string()));
    }
}
