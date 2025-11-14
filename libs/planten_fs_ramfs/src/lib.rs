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
}