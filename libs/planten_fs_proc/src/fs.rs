use planten_fs_core::{FsServer, Inode};
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System};

const PROC_FILES: [&str; 2] = ["cmdline", "status"];

/// A 9P filesystem that exposes process information from the underlying OS.
/// It uses the `sysinfo` crate to provide a cross-platform view of processes.
pub struct ProcFs {
    sys: System,
}

impl ProcFs {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        ProcFs { sys }
    }

    fn list_pids(&self) -> io::Result<Vec<String>> {
        let pids = self.sys.processes().keys().map(|p| p.to_string()).collect();
        Ok(pids)
    }

    fn pid_exists(&self, pid_str: &str) -> bool {
        if let Ok(pid_val) = pid_str.parse::<usize>() {
            self.sys.process(Pid::from(pid_val)).is_some()
        } else {
            false
        }
    }

    fn read_cmdline(&self, pid_str: &str) -> io::Result<Vec<u8>> {
        if let Ok(pid_val) = pid_str.parse::<usize>() {
            if let Some(process) = self.sys.process(Pid::from(pid_val)) {
                let mut line = process.cmd().join(" ");
                line.push('\n');
                return Ok(line.into_bytes());
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "process not found",
        ))
    }

    fn read_status(&self, pid_str: &str) -> io::Result<Vec<u8>> {
        if let Ok(pid_val) = pid_str.parse::<usize>() {
            if let Some(process) = self.sys.process(Pid::from(pid_val)) {
                let status = format!(
                    "pid {}\nuser {}\ncommand {}\ncpu {}\nmem {}\n",
                    process.pid(),
                    process.user_id().map_or("?".to_string(), |u| u.to_string()),
                    process.name(),
                    process.cpu_usage(),
                    process.memory(),
                );
                return Ok(status.into_bytes());
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "process not found",
        ))
    }

    fn directory_listing(&self, entries: &[String]) -> Vec<u8> {
        let mut buf = entries.join("\n");
        buf.push('\n');
        buf.into_bytes()
    }
}

impl FsServer for ProcFs {
    fn walk(&self, path: &str) -> Option<Vec<String>> {
        let components: Vec<&str> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        match components.as_slice() {
            [] => self.list_pids().ok(),
            [pid] if self.pid_exists(pid) => {
                Some(PROC_FILES.iter().map(|s| s.to_string()).collect())
            }
            [pid, file] if PROC_FILES.contains(file) && self.pid_exists(pid) => Some(vec![]),
            _ => None,
        }
    }

    fn open(&self, path: &str) -> Option<()> {
        let components: Vec<&str> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        match components.as_slice() {
            [pid, file] if PROC_FILES.contains(file) && self.pid_exists(pid) => Some(()),
            [pid] if self.pid_exists(pid) => Some(()),
            [] => Some(()),
            _ => None,
        }
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let components: Vec<&str> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        match components.as_slice() {
            [] => self
                .list_pids()
                .ok()
                .map(|pids| self.directory_listing(&pids)),
            [pid] => {
                if self.pid_exists(pid) {
                    let entries = PROC_FILES.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    Some(self.directory_listing(&entries))
                } else {
                    None
                }
            }
            [pid, file] if self.pid_exists(pid) && *file == "cmdline" => {
                self.read_cmdline(pid).ok()
            }
            [pid, file] if self.pid_exists(pid) && *file == "status" => self.read_status(pid).ok(),
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
        let components: Vec<&str> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        match components.as_slice() {
            [] => {
                let mut inode = Inode::new("/", 0o555 | 0x80000000, "root", "root");
                inode.atime = now;
                inode.mtime = now;
                Some(inode)
            }
            [pid] if self.pid_exists(pid) => {
                let mut inode = Inode::new(pid, 0o555 | 0x80000000, "root", "root");
                inode.atime = now;
                inode.mtime = now;
                Some(inode)
            }
            [pid, file] if self.pid_exists(pid) && *file == "cmdline" => {
                if let Ok(data) = self.read_cmdline(pid) {
                    let mut inode = Inode::new("cmdline", 0o444, "root", "root");
                    inode.data = data;
                    inode.atime = now;
                    inode.mtime = now;
                    Some(inode)
                } else {
                    None
                }
            }
            [pid, file] if self.pid_exists(pid) && *file == "status" => {
                if let Ok(data) = self.read_status(pid) {
                    let mut inode = Inode::new("status", 0o444, "root", "root");
                    inode.data = data;
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
