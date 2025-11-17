use planten_fs_core::{FsServer, Inode};
use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessStatus, System};

const PROC_FILES: [&str; 4] = ["cmdline", "status", "stat", "info"];

#[derive(Copy, Clone, Debug)]
enum ProcFile {
    Cmdline,
    Status,
    Stat,
    Info,
}

impl ProcFile {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "cmdline" => Some(ProcFile::Cmdline),
            "status" => Some(ProcFile::Status),
            "stat" => Some(ProcFile::Stat),
            "info" => Some(ProcFile::Info),
            _ => None,
        }
    }
}

/// A 9P filesystem that exposes process information from the underlying OS.
/// It uses the `sysinfo` crate to provide a cross-platform view of processes.
pub struct ProcFs {
    sys: RefCell<System>,
}

impl ProcFs {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        ProcFs {
            sys: RefCell::new(sys),
        }
    }

    fn directory_listing(entries: &[String]) -> Vec<u8> {
        let mut buf = entries.join("\n");
        buf.push('\n');
        buf.into_bytes()
    }

    fn list_pids(&self) -> Vec<String> {
        let mut sys = self.sys.borrow_mut();
        sys.refresh_processes();
        let mut pids: Vec<String> = sys.processes().keys().map(|p| p.to_string()).collect();
        pids.sort();
        pids
    }

    fn pid_exists(&self, pid_str: &str) -> bool {
        if let Ok(pid_val) = pid_str.parse::<usize>() {
            let pid = Pid::from(pid_val);
            let mut sys = self.sys.borrow_mut();
            sys.refresh_processes();
            sys.process(pid).is_some()
        } else {
            false
        }
    }

    fn with_process<T>(&self, pid: usize, f: impl FnOnce(&sysinfo::Process) -> T) -> Option<T> {
        let mut sys = self.sys.borrow_mut();
        sys.refresh_processes();
        let process = sys.process(Pid::from(pid))?;
        Some(f(process))
    }

    fn write_cmdline(process: &sysinfo::Process) -> Vec<u8> {
        let mut data = process.cmd().join("\x00").into_bytes();
        if !data.ends_with(&[0]) {
            data.push(0);
        }
        data
    }

    fn write_status(process: &sysinfo::Process) -> Vec<u8> {
        let status = format!(
            "Name: {}\nPid: {}\nStatus: {:?}\nPPid: {:?}\nCmd: {}\nCPU: {}\nMemory: {}\n",
            process.name(),
            process.pid(),
            process.status(),
            process.parent().map(|p| p.as_u32()),
            process.cmd().join(" "),
            process.cpu_usage(),
            process.memory(),
        );
        status.into_bytes()
    }

    fn write_stat(process: &sysinfo::Process) -> Vec<u8> {
        let state_char = match process.status() {
            ProcessStatus::Run => 'R',
            ProcessStatus::Sleep => 'S',
            ProcessStatus::Idle => 'I',
            ProcessStatus::Stop => 'T',
            ProcessStatus::Zombie => 'Z',
            ProcessStatus::Unknown(_) => 'U',
            _ => 'U',
        };
        let stat = format!(
            "{} ({}) {} {} {} {}\n",
            process.pid(),
            process.name(),
            state_char,
            process.parent().map_or(0, |p| p.as_u32()),
            process.memory(),
            process.cpu_usage()
        );
        stat.into_bytes()
    }

    fn write_info(process: &sysinfo::Process) -> Vec<u8> {
        let info = format!(
            "pid {}\nname {}\nstate {:?}\ncwd {:?}\nexe {:?}\nusermode {}\n",
            process.pid(),
            process.name(),
            process.status(),
            process.cwd(),
            process.exe(),
            process
                .user_id()
                .map_or("unknown".to_string(), |u| u.to_string()),
        );
        info.into_bytes()
    }
}

impl FsServer for ProcFs {
    fn walk(&self, path: &str) -> Option<Vec<String>> {
        let components: Vec<&str> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        match components.as_slice() {
            [] => Some(self.list_pids()),
            [pid] if self.pid_exists(pid) => {
                Some(PROC_FILES.iter().map(|s| s.to_string()).collect())
            }
            [pid, file] if self.pid_exists(pid) && PROC_FILES.contains(file) => Some(vec![]),
            _ => None,
        }
    }

    fn open(&self, path: &str) -> Option<()> {
        let components: Vec<&str> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        match components.as_slice() {
            [] => Some(()),
            [pid] if self.pid_exists(pid) => Some(()),
            [pid, file] if self.pid_exists(pid) && PROC_FILES.contains(file) => Some(()),
            _ => None,
        }
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let components: Vec<&str> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        match components.as_slice() {
            [] => Some(Self::directory_listing(&self.list_pids())),
            [pid] if self.pid_exists(pid) => {
                let entries = PROC_FILES.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                Some(Self::directory_listing(&entries))
            }
            [pid, file] if self.pid_exists(pid) => {
                if let Ok(pid_val) = pid.parse::<usize>() {
                    let proc_file = ProcFile::from_name(file)?;
                    match proc_file {
                        ProcFile::Cmdline => self.with_process(pid_val, Self::write_cmdline),
                        ProcFile::Status => self.with_process(pid_val, Self::write_status),
                        ProcFile::Stat => self.with_process(pid_val, Self::write_stat),
                        ProcFile::Info => self.with_process(pid_val, Self::write_info),
                    }
                } else {
                    None
                }
            }
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
            [pid, file] if self.pid_exists(pid) && PROC_FILES.contains(file) => {
                let data = self.read(path)?;
                let mut inode = Inode::new(file, 0o444, "root", "root");
                inode.data = data;
                inode.atime = now;
                inode.mtime = now;
                Some(inode)
            }
            _ => None,
        }
    }

    fn wstat(&mut self, _path: &str, _inode: Inode) -> Option<()> {
        None
    }
}
