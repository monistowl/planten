use std::collections::HashMap;
use planten_fs_core::FsServer;

pub struct Inode {
    pub name: String,
    pub data: Vec<u8>,
    pub children: HashMap<String, Inode>,
}

impl Inode {
    pub fn new(name: &str) -> Self {
        Inode {
            name: name.to_string(),
            data: Vec::new(),
            children: HashMap::new(),
        }
    }
}

pub struct RamFs {
    root: Inode,
}

impl RamFs {
    pub fn new() -> Self {
        RamFs {
            root: Inode::new("/"),
        }
    }

    pub fn create_file(&mut self, path: &str, data: &[u8]) {
        let mut current = &mut self.root;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (i, component) in components.iter().enumerate() {
            if i == components.len() - 1 {
                let mut file = Inode::new(component);
                file.data = data.to_vec();
                current.children.insert(component.to_string(), file);
            } else {
                current = current.children.entry(component.to_string()).or_insert_with(|| Inode::new(component));
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
}

impl FsServer for RamFs {
    fn walk(&self, path: &str) -> Option<Vec<String>> {
        self.list_dir(path).map(|v| v.into_iter().map(|s| s.to_string()).collect())
    }

    fn open(&self, path: &str) -> Option<()> {
        self.read_file(path).map(|_| ())
    }

    fn read(&self, path: &str) -> Option<&[u8]> {
        self.read_file(path)
    }

    fn write(&mut self, path: &str, data: &[u8]) -> Option<()> {
        self.create_file(path, data);
        Some(())
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

    fn stat(&self, path: &str) -> Option<()> {
        self.read_file(path).map(|_| ())
    }

    fn wstat(&mut self, _path: &str) -> Option<()> {
        Some(())
    }
}