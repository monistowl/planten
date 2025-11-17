use planten_fs_core::{FsServer, Inode};
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, Process, ProcessStatus, System};

const PROC_ENTRIES: &[ProcEntry] = &[
    ProcEntry::file("cmdline", ProcFile::Cmdline),
    ProcEntry::file("status", ProcFile::Status),
    ProcEntry::file("stat", ProcFile::Stat),
    ProcEntry::file("info", ProcFile::Info),
    ProcEntry::file("statm", ProcFile::Statm),
    ProcEntry::file("mounts", ProcFile::Mounts),
];

const PROC_DIRS: &[ProcDir] = &[ProcDir::Fd, ProcDir::Task];

const FD_ENTRIES: &[&str] = &["0", "1", "2"];
const TASK_ENTRIES: &[&str] = &["self"];

#[derive(Copy, Clone, Debug)]
enum ProcFile {
    Cmdline,
    Status,
    Stat,
    Info,
    Statm,
    Mounts,
}

#[derive(Copy, Clone, Debug)]
enum ProcDir {
    Fd,
    Task,
}

impl ProcDir {
    fn name(&self) -> &'static str {
        match self {
            ProcDir::Fd => "fd",
            ProcDir::Task => "task",
        }
    }

    fn entries(&self) -> Vec<String> {
        match self {
            ProcDir::Fd => FD_ENTRIES.iter().map(|s| s.to_string()).collect(),
            ProcDir::Task => TASK_ENTRIES.iter().map(|s| s.to_string()).collect(),
        }
    }
}

struct ProcEntry {
    name: &'static str,
    kind: EntryKind,
}

#[derive(Copy, Clone)]
enum EntryKind {
    File(ProcFile),
    Dir(ProcDir),
}

impl ProcEntry {
    const fn file(name: &'static str, file: ProcFile) -> Self {
        ProcEntry {
            name,
            kind: EntryKind::File(file),
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

    fn pid_exists(&self, pid: &str) -> bool {
        if let Ok(pid_val) = pid.parse::<usize>() {
            let pid = Pid::from(pid_val);
            let mut sys = self.sys.borrow_mut();
            sys.refresh_processes();
            sys.process(pid).is_some()
        } else {
            false
        }
    }

    fn with_process<T>(&self, pid: usize, f: impl FnOnce(&Process) -> T) -> Option<T> {
        let mut sys = self.sys.borrow_mut();
        sys.refresh_processes();
        let process = sys.process(Pid::from(pid))?;
        Some(f(process))
    }

    fn process_entry_names(&self) -> Vec<String> {
        let mut names: Vec<String> = PROC_ENTRIES
            .iter()
            .map(|entry| entry.name.to_string())
            .collect();
        names.extend(PROC_DIRS.iter().map(|dir| dir.name().to_string()));
        names
    }

    fn find_entry_kind(name: &str) -> Option<EntryKind> {
        for entry in PROC_ENTRIES {
            if entry.name == name {
                return Some(entry.kind);
            }
        }
        for dir in PROC_DIRS {
            if dir.name() == name {
                return Some(EntryKind::Dir(*dir));
            }
        }
        None
    }

    fn read_file_entry(&self, pid: usize, file: ProcFile) -> Option<Vec<u8>> {
        match file {
            ProcFile::Cmdline => self.with_process(pid, Self::write_cmdline),
            ProcFile::Status => self.with_process(pid, Self::write_status),
            ProcFile::Stat => self.with_process(pid, Self::write_stat),
            ProcFile::Info => self.with_process(pid, Self::write_info),
            ProcFile::Statm => self.with_process(pid, Self::write_statm),
            ProcFile::Mounts => self.with_process(pid, Self::write_mounts),
        }
    }

    fn write_cmdline(process: &Process) -> Vec<u8> {
        let mut data = process.cmd().join("\x00").into_bytes();
        if !data.ends_with(&[0]) {
            data.push(0);
        }
        data
    }

    fn write_status(process: &Process) -> Vec<u8> {
        format!(
            "Name: {}\nPid: {}\nStatus: {:?}\nPPid: {:?}\nCmd: {}\nCPU: {}\nMemory: {}\n",
            process.name(),
            process.pid(),
            process.status(),
            process.parent().map(|p| p.as_u32()),
            process.cmd().join(" "),
            process.cpu_usage(),
            process.memory(),
        )
        .into_bytes()
    }

    fn write_stat(process: &Process) -> Vec<u8> {
        let state_char = match process.status() {
            ProcessStatus::Run => 'R',
            ProcessStatus::Sleep => 'S',
            ProcessStatus::Idle => 'I',
            ProcessStatus::Stop => 'T',
            ProcessStatus::Zombie => 'Z',
            ProcessStatus::Unknown(_) => 'U',
            _ => 'U',
        };
        format!(
            "{} ({}) {} {} {} {}\n",
            process.pid(),
            process.name(),
            state_char,
            process.parent().map_or(0, |p| p.as_u32()),
            process.memory(),
            process.cpu_usage()
        )
        .into_bytes()
    }

    fn write_info(process: &Process) -> Vec<u8> {
        let cwd = process
            .cwd()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let exe = process
            .exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        format!(
            "pid {}\nname {}\nstate {:?}\ncwd {}\nexe {}\nusermode {}\n",
            process.pid(),
            process.name(),
            process.status(),
            cwd,
            exe,
            process
                .user_id()
                .map_or("unknown".to_string(), |u| u.to_string()),
        )
        .into_bytes()
    }

    fn write_statm(process: &Process) -> Vec<u8> {
        let size = process.virtual_memory();
        let resident = process.memory();
        let share = 0;
        let text = 0;
        let lib = 0;
        let data = process.memory();
        let dt = 0;
        format!(
            "{} {} {} {} {} {} {}\n",
            size, resident, share, text, lib, data, dt
        )
        .into_bytes()
    }

    fn write_mounts(process: &Process) -> Vec<u8> {
        if cfg!(target_os = "linux") {
            let path = Path::new("/proc")
                .join(process.pid().to_string())
                .join("mounts");
            if let Ok(content) = fs::read_to_string(path) {
                return content.into_bytes();
            }
        }
        "proc /proc proc rw 0 0\n".as_bytes().to_vec()
    }

    fn read_fd_entry(&self, pid: usize, name: &str) -> Option<Vec<u8>> {
        if FD_ENTRIES.contains(&name) {
            Some(format!("fd {}/{} -> placeholder\n", pid, name).into_bytes())
        } else {
            None
        }
    }

    fn read_task_entry(&self, pid: usize, name: &str) -> Option<Vec<u8>> {
        if TASK_ENTRIES.contains(&name) {
            self.read_file_entry(pid, ProcFile::Status)
        } else {
            None
        }
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
            [pid] if self.pid_exists(pid) => Some(self.process_entry_names()),
            [pid, name] if self.pid_exists(pid) => {
                if let Some(entry) = Self::find_entry_kind(name) {
                    match entry {
                        EntryKind::Dir(dir) => Some(dir.entries()),
                        EntryKind::File(_) => Some(vec![]),
                    }
                } else {
                    None
                }
            }
            [pid, dir, item] if self.pid_exists(pid) => {
                if let Some(entry) = PROC_DIRS.iter().find(|d| d.name() == *dir) {
                    if let ProcDir::Fd = entry {
                        if FD_ENTRIES.contains(item) {
                            Some(vec![])
                        } else {
                            None
                        }
                    } else if let ProcDir::Task = entry {
                        if TASK_ENTRIES.contains(item) {
                            Some(vec![])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
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
            [pid, name] if self.pid_exists(pid) => {
                Self::find_entry_kind(name).map(|entry| match entry {
                    EntryKind::Dir(_) => (),
                    EntryKind::File(_) => (),
                })
            }
            [pid, dir, entry] if self.pid_exists(pid) => {
                if PROC_DIRS.iter().any(|d| d.name() == *dir) {
                    Some(())
                } else {
                    None
                }
            }
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
                let entries = self.process_entry_names();
                Some(Self::directory_listing(&entries))
            }
            [pid, name] if self.pid_exists(pid) => {
                if let Some(entry) = Self::find_entry_kind(name) {
                    match entry {
                        EntryKind::Dir(dir) => Some(Self::directory_listing(&dir.entries())),
                        EntryKind::File(file) => {
                            if let Ok(pid_val) = pid.parse::<usize>() {
                                self.read_file_entry(pid_val, file)
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                }
            }
            [pid, dir, entry] if self.pid_exists(pid) => {
                if let Ok(pid_val) = pid.parse::<usize>() {
                    if let Some(proc_dir) = PROC_DIRS.iter().find(|d| d.name() == *dir) {
                        match proc_dir {
                            ProcDir::Fd => self.read_fd_entry(pid_val, entry),
                            ProcDir::Task => self.read_task_entry(pid_val, entry),
                        }
                    } else {
                        None
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
            [pid, name] if self.pid_exists(pid) => {
                if let Some(entry) = Self::find_entry_kind(name) {
                    match entry {
                        EntryKind::Dir(_) => {
                            let mut inode = Inode::new(name, 0o555 | 0x80000000, "root", "root");
                            inode.atime = now;
                            inode.mtime = now;
                            Some(inode)
                        }
                        EntryKind::File(_) => {
                            if let Some(data) = self.read(path) {
                                let mut inode = Inode::new(name, 0o444, "root", "root");
                                inode.data = data;
                                inode.atime = now;
                                inode.mtime = now;
                                Some(inode)
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                }
            }
            [pid, dir, entry] if self.pid_exists(pid) => {
                if let Ok(pid_val) = pid.parse::<usize>() {
                    if let Some(proc_dir) = PROC_DIRS.iter().find(|d| d.name() == *dir) {
                        let data = match proc_dir {
                            ProcDir::Fd => self.read_fd_entry(pid_val, entry),
                            ProcDir::Task => self.read_task_entry(pid_val, entry),
                        };
                        if let Some(data) = data {
                            let mut inode = Inode::new(entry, 0o444, "root", "root");
                            inode.data = data;
                            inode.atime = now;
                            inode.mtime = now;
                            return Some(inode);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn wstat(&mut self, _path: &str, _inode: Inode) -> Option<()> {
        None
    }
}
