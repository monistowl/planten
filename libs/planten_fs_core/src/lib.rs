pub trait FsServer {
    fn walk(&self, path: &str) -> Option<Vec<String>>;
    fn open(&self, path: &str) -> Option<()>;
    fn read(&self, path: &str) -> Option<&[u8]>;
    fn write(&mut self, path: &str, data: &[u8]) -> Option<()>;
    fn clunk(&self, path: &str) -> Option<()>;
    fn remove(&mut self, path: &str) -> Option<()>;
    fn stat(&self, path: &str) -> Option<()>;
    fn wstat(&mut self, path: &str) -> Option<()>;
}
