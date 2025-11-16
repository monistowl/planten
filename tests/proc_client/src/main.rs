use planten_9p::P9Client;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = "127.0.0.1:5641";
    println!("Connecting to ProcFs 9P server at {}", addr);

    let mut client = P9Client::new(addr)?;
    println!("Connected.");

    // 1. Version exchange
    let negotiated_version = client.version(8192, "9P2000")?;
    println!("Negotiated version: {}", negotiated_version);

    // 2. Attach to the root
    let fid = 0;
    client.attach(fid, None, "user", "none")?;
    println!("Attached to root with fid {}", fid);

    // 3. Walk to list process IDs (directories)
    let walk_fid = 1;
    let pids = client.walk(fid, walk_fid, &[])?;
    println!("Walked root, found {} PIDs (directories)", pids);

    // 4. Walk into a specific process ID directory (e.g., the first one)
    if pids > 0 {
        let first_pid_dir_name = {
            // To get the actual PID name, we need to stat the directory.
            // For simplicity, let's assume the first entry is a PID.
            // In a real client, we'd walk and then stat to get the name.
            // For now, we'll just try to walk to a common PID like "1" (init)
            // or assume the server returns PIDs directly in walk.
            // Since our server returns PIDs directly, we can try to walk to one.
            // This part needs refinement if the server doesn't return actual names in walk.
            // For now, let's just try to walk to "1" (init process)
            "1"
        };

        let pid_dir_fid = 2;
        let walked_pid_dir = client.walk(fid, pid_dir_fid, &[first_pid_dir_name])?;
        println!(
            "Walked into PID directory '{}', walked {} elements",
            first_pid_dir_name, walked_pid_dir
        );

        // 5. Walk to the "info" file within a process ID directory
        let info_file_fid = 3;
        let walked_info_file = client.walk(pid_dir_fid, info_file_fid, &["info"])?;
        println!(
            "Walked to 'info' file in PID '{}', walked {} elements",
            first_pid_dir_name, walked_info_file
        );

        // 6. Open the "info" file
        let iounit = client.open(info_file_fid, 0)?;
        println!("Opened 'info' file, iounit: {}", iounit);

        // 7. Read the content of the "info" file
        let content = client.read(info_file_fid, 0, iounit)?;
        let content_str = String::from_utf8_lossy(&content);
        println!("Content of 'info' file:\n{}", content_str);

        // 8. Clunk all FIDs
        client.clunk(info_file_fid)?;
        client.clunk(pid_dir_fid)?;
        client.clunk(walk_fid)?;
    }
    client.clunk(fid)?;
    println!("All FIDs clunked. Test client finished successfully.");

    Ok(())
}
