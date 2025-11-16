use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum Mount {
    Bind { path: String },
    Union { paths: Vec<String> },
    P9 { addr: String, path: String },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct MountEntry {
    pub target: String,
    pub mount: Mount,
}

#[derive(Serialize, Deserialize)]
pub struct Namespace {
    mounts: Vec<MountEntry>,
}

#[derive(Debug)]
pub enum MountPlan {
    Bind { path: String },
    Union { paths: Vec<String> },
    P9 { addr: String, path: String },
}

impl Namespace {
    pub fn new() -> Self {
        Namespace { mounts: Vec::new() }
    }

    fn add_mount_entry(&mut self, target: &str, mount: Mount) {
        self.mounts.push(MountEntry {
            target: target.to_string(),
            mount,
        });
    }

    pub fn bind(&mut self, new: &str, old: &str) {
        self.add_mount_entry(
            new,
            Mount::Bind {
                path: old.to_string(),
            },
        );
    }

    pub fn union(&mut self, new: &str, old: &str) {
        self.add_mount_entry(
            new,
            Mount::Union {
                paths: vec![old.to_string()],
            },
        );
    }

    pub fn union_multi(&mut self, new: &str, old_paths: &[&str]) {
        let paths = old_paths.iter().map(|p| p.to_string()).collect();
        self.add_mount_entry(new, Mount::Union { paths });
    }

    pub fn p9(&mut self, new: &str, addr: &str, path: &str) {
        self.add_mount_entry(
            new,
            Mount::P9 {
                addr: addr.to_string(),
                path: path.to_string(),
            },
        );
    }

    pub fn mounts(&self) -> &[MountEntry] {
        &self.mounts
    }

    pub fn mount_plan(&self) -> Vec<(String, MountPlan)> {
        let mut plan = Vec::new();
        for entry in &self.mounts {
            match &entry.mount {
                Mount::Bind { path } => {
                    plan.push((entry.target.clone(), MountPlan::Bind { path: path.clone() }));
                }
                Mount::P9 { addr, path } => {
                    plan.push((
                        entry.target.clone(),
                        MountPlan::P9 {
                            addr: addr.clone(),
                            path: path.clone(),
                        },
                    ));
                }
                Mount::Union { paths } => match plan.last_mut() {
                    Some((target, MountPlan::Union { paths: existing }))
                        if target == &entry.target =>
                    {
                        existing.extend(paths.clone());
                    }
                    _ => {
                        plan.push((
                            entry.target.clone(),
                            MountPlan::Union {
                                paths: paths.clone(),
                            },
                        ));
                    }
                },
            }
        }
        plan
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, file_path: P) -> io::Result<()> {
        let path = file_path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(self)?;
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(file_path: P) -> io::Result<Self> {
        let path = file_path.as_ref();
        if !path.exists() {
            return Ok(Namespace::new());
        }
        let contents = fs::read_to_string(path)?;
        let namespace = serde_json::from_str(&contents)?;
        Ok(namespace)
    }

    pub fn storage_path() -> io::Result<PathBuf> {
        if let Some(home_dir) = env::var_os("HOME") {
            let mut path = PathBuf::from(home_dir);
            path.push(".planten");
            if fs::create_dir_all(&path).is_ok() {
                path.push("ns.json");
                return Ok(path);
            }
        }
        let mut fallback = PathBuf::from("/srv");
        fallback.push("planten");
        fs::create_dir_all(&fallback)?;
        fallback.push("ns.json");
        Ok(fallback)
    }

    pub fn load_from_storage() -> io::Result<Self> {
        let path = Self::storage_path()?;
        Self::load_from_file(path)
    }

    pub fn save_to_storage(&self) -> io::Result<()> {
        let path = Self::storage_path()?;
        self.save_to_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use std::fs;
    use tempfile::tempdir;

    fn setup_home(tmp: &tempfile::TempDir) {
        unsafe { env::set_var("HOME", tmp.path()) };
    }

    #[test]
    fn storage_path_prefers_home() {
        let tmp = tempdir().unwrap();
        setup_home(&tmp);
        let storage = Namespace::storage_path().unwrap();
        assert!(storage.ends_with(".planten/ns.json"));
        assert!(storage.starts_with(tmp.path()));
    }

    #[test]
    fn save_and_load_round_trip_preserves_order() {
        let tmp = tempdir().unwrap();
        setup_home(&tmp);
        let mut ns = Namespace::new();
        ns.bind("/a", "/old");
        ns.union("/union", "/first");
        ns.union_multi("/union", &["/second", "/third"]);
        ns.p9("/p9", "host", "/path");
        let storage_path_before = Namespace::storage_path().unwrap();
        ns.save_to_storage().unwrap();
        setup_home(&tmp);
        let storage_path_after = Namespace::storage_path().unwrap();
        assert_eq!(storage_path_before, storage_path_after);
        let contents = fs::read_to_string(&storage_path_after).unwrap();
        assert!(!contents.trim().is_empty());

        let loaded = Namespace::load_from_storage().unwrap();
        assert_eq!(ns.mounts().len(), loaded.mounts().len());
        assert_eq!(ns.mounts(), loaded.mounts());
    }

    #[test]
    fn mount_plan_merges_unions() {
        let mut ns = Namespace::new();
        ns.union("/union", "/first");
        ns.union("/union", "/second");
        ns.union("/other", "/x");

        let plan = ns.mount_plan();
        assert_eq!(plan.len(), 2);

        if let MountPlan::Union { paths } = &plan[0].1 {
            assert_eq!(paths, &vec!["/first".to_string(), "/second".to_string()]);
        } else {
            panic!("expected union plan");
        }

        if let MountPlan::Union { paths } = &plan[1].1 {
            assert_eq!(paths, &vec!["/x".to_string()]);
        } else {
            panic!("expected union plan");
        }
    }
}
