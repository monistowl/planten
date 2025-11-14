use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;

#[derive(Serialize, Deserialize)]
pub enum Mount {
    Bind { path: String },
    Union { paths: Vec<String> },
    P9 { addr: String, path: String },
}

#[derive(Serialize, Deserialize)]
pub struct Namespace {
    mounts: HashMap<String, Mount>,
}

impl Namespace {
    pub fn new() -> Self {
        Namespace {
            mounts: HashMap::new(),
        }
    }

    pub fn add_mount(&mut self, path: &str, mount: Mount) {
        self.mounts.insert(path.to_string(), mount);
    }

    pub fn bind(&mut self, new: &str, old: &str) {
        self.add_mount(
            new,
            Mount::Bind {
                path: old.to_string(),
            },
        );
    }

    pub fn union(&mut self, new: &str, old: &str) {
        let mount = self
            .mounts
            .entry(new.to_string())
            .or_insert(Mount::Union { paths: vec![] });
        if let Mount::Union { paths } = mount {
            paths.push(old.to_string());
        }
    }

    pub fn p9(&mut self, new: &str, addr: &str, path: &str) {
        self.add_mount(
            new,
            Mount::P9 {
                addr: addr.to_string(),
                path: path.to_string(),
            },
        );
    }

    pub fn mounts(&self) -> &HashMap<String, Mount> {
        &self.mounts
    }

    pub fn save_to_file(&self, file_path: &str) -> io::Result<()> {
        let serialized = serde_json::to_string_pretty(self)?;
        fs::write(file_path, serialized)?;
        Ok(())
    }

    pub fn load_from_file(file_path: &str) -> io::Result<Self> {
        if !fs::metadata(file_path).is_ok() {
            return Ok(Namespace::new());
        }
        let contents = fs::read_to_string(file_path)?;
        let namespace = serde_json::from_str(&contents)?;
        Ok(namespace)
    }
}
