use std::io;
use std::net::SocketAddr;

use planten_fs_proc::server;

fn main() -> io::Result<()> {
    let addr: SocketAddr = "127.0.0.1:5641".parse().expect("valid address");
    println!("ProcFs 9P server listening on {}", addr);
    server::start_server(addr.to_string().as_str())?;
    Ok(())
}
