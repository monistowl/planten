use anyhow::{Context, Result};
use planten_9p::P9Client;
use std::env;

fn main() -> Result<()> {
    let addr = env::var("PLAN9_QEMU_ADDR").unwrap_or_else(|_| "127.0.0.1:1564".into());
    println!("Connecting to Plan 9 guest at {}", addr);

    let mut client = P9Client::new(&addr)
        .with_context(|| format!("failed to connect to Plan 9 guest at {}", addr))?;

    let version = client.version(131_072, "9P2000")?;
    println!("Negotiated version {}", version);

    let root_fid = 0;
    client
        .attach(root_fid, None, "guest", "")
        .context("attach failed")?;

    let walk_fid = 1;
    let walked = client.walk(root_fid, walk_fid, &[])?;
    println!("Walked root, got {} qids", walked);

    let _iounit = client.open(walk_fid, 0).context("open root")?;
    let data = client.read(walk_fid, 0, 256)?;
    println!(
        "Read {} bytes from root: {:?}",
        data.len(),
        &data[..data.len().min(32)]
    );

    client.clunk(walk_fid)?;
    client.clunk(root_fid)?;
    println!("Plan 9 guest handshake succeeded");

    Ok(())
}
