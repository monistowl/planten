
use std::collections::HashMap;

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
