pub trait FsServer {
    fn walk(&self, path: &str) -> Option<Vec<String>>;
    fn open(&self, path: &str) -> Option<()>;
    fn read(&self, path: &str) -> Option<&[u8]>;
    fn write(&mut self, path: &str, offset: u64, data: &[u8]) -> Option<u32>;
    fn clunk(&self, path: &str) -> Option<()>;
    fn remove(&mut self, path: &str) -> Option<()>;
    fn stat(&self, path: &str) -> Option<()>;
    fn wstat(&mut self, path: &str) -> Option<()>;
}
