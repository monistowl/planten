use std::collections::HashMap;

pub enum Mount {
    Bind {
        path: String,
    },
    Union {
        paths: Vec<String>,
    },
}

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
        self.add_mount(new, Mount::Bind { path: old.to_string() });
    }

    pub fn union(&mut self, new: &str, old: &str) {
        let mount = self.mounts.entry(new.to_string()).or_insert(Mount::Union { paths: vec![] });
        if let Mount::Union { paths } = mount {
            paths.push(old.to_string());
        }
    }

    pub fn mounts(&self) -> &HashMap<String, Mount> {
        &self.mounts
    }
}