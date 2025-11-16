use planten_fs_core::{FsServer, Inode};

pub struct RamFs {
    root: Inode,
}

impl RamFs {
    pub fn new() -> Self {
        RamFs {
            root: Inode::new("/", 0o755 | 0x80000000, "user", "group"),
        }
    }

    pub fn wstat_from_stat(&mut self, path: &str, stat: &planten_9p::Stat) -> Option<()> {
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if components.is_empty() {
            // root
            if stat.mode != !0u32 {
                self.root.mode = stat.mode;
            }
            if stat.mtime != !0u32 {
                self.root.mtime = stat.mtime;
            }
            if !stat.gid.is_empty() {
                self.root.gid = stat.gid.clone();
            }
            return Some(());
        }

        let (filename, path_parts) = components.split_last()?;
        let mut current = &mut self.root;
        for part in path_parts {
            current = current.children.get_mut(*part)?;
        }

        let mut inode = current.children.get(filename)?.clone();

        if stat.mode != !0u32 {
            inode.mode = stat.mode;
        }
        if stat.mtime != !0u32 {
            inode.mtime = stat.mtime;
        }
        if stat.length != !0u64 {
            inode.data.resize(stat.length as usize, 0);
        }
        if !stat.gid.is_empty() {
            inode.gid = stat.gid.clone();
        }

        if !stat.name.is_empty() && stat.name != *filename {
            // rename
            if current.children.contains_key(&stat.name) {
                return None; // exists
            }
            inode.name = stat.name.clone();
            current.children.remove(*filename);
            current.children.insert(inode.name.clone(), inode);
        } else {
            *current.children.get_mut(filename).unwrap() = inode;
        }

        Some(())
    }

    pub fn create_file(&mut self, path: &str, data: &[u8]) {
        let mut current = &mut self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (i, component) in components.iter().enumerate() {
            if i == components.len() - 1 {
                let mut file = Inode::new(component, 0o644, "user", "group");
                file.data = data.to_vec();
                current.children.insert(component.to_string(), file);
            } else {
                current = current
                    .children
                    .entry(component.to_string())
                    .or_insert_with(|| Inode::new(component, 0o755 | 0x80000000, "user", "group"));
            }
        }
    }

    pub fn read_file(&self, path: &str) -> Option<&[u8]> {
        let mut current = &self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (i, component) in components.iter().enumerate() {
            if let Some(node) = current.children.get(*component) {
                if i == components.len() - 1 {
                    return Some(&node.data);
                }
                current = node;
            } else {
                return None;
            }
        }
        None
    }

    pub fn list_dir(&self, path: &str) -> Option<Vec<&str>> {
        let mut current = &self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for component in components {
            if let Some(node) = current.children.get(component) {
                current = node;
            } else {
                return None;
            }
        }
        Some(current.children.keys().map(|s| s.as_str()).collect())
    }

    pub fn create_dir(&mut self, path: &str) {
        let mut current = &mut self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (i, component) in components.iter().enumerate() {
            if i == components.len() - 1 {
                current
                    .children
                    .entry(component.to_string())
                    .or_insert_with(|| Inode::new(component, 0o755 | 0x80000000, "user", "group"));
            } else {
                current = current
                    .children
                    .entry(component.to_string())
                    .or_insert_with(|| Inode::new(component, 0o755 | 0x80000000, "user", "group"));
            }
        }
    }
}

impl FsServer for RamFs {
    fn walk(&self, path: &str) -> Option<Vec<String>> {
        self.list_dir(path)
            .map(|v| v.into_iter().map(|s| s.to_string()).collect())
    }

    fn open(&self, path: &str) -> Option<()> {
        self.read_file(path).map(|_| ())
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.read_file(path).map(|data| data.to_vec())
    }

    fn write(&mut self, path: &str, offset: u64, data: &[u8]) -> Option<u32> {
        let mut current = &mut self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let (filename, path_parts) = components.split_last()?;

        for part in path_parts {
            current = current.children.get_mut(*part)?;
        }

        let node = current.children.get_mut(*filename)?;
        let start = offset as usize;
        let end = start + data.len();
        if end > node.data.len() {
            node.data.resize(end, 0);
        }
        node.data[start..end].copy_from_slice(data);
        Some(data.len() as u32)
    }

    fn clunk(&self, _path: &str) -> Option<()> {
        Some(())
    }

    fn remove(&mut self, path: &str) -> Option<()> {
        let mut current = &mut self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (i, component) in components.iter().enumerate() {
            if i == components.len() - 1 {
                return current.children.remove(*component).map(|_| ());
            }
            if let Some(node) = current.children.get_mut(*component) {
                current = node;
            } else {
                return None;
            }
        }
        None
    }

    fn stat(&self, path: &str) -> Option<Inode> {
        let mut current = &self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for component in components {
            if let Some(node) = current.children.get(component) {
                current = node;
            } else {
                return None;
            }
        }
        Some(current.clone())
    }

    fn wstat(&mut self, _path: &str, _inode: Inode) -> Option<()> {
        Some(())
    }
}

pub mod server;
