
use std::env;
use std::process::Command;
use planten_ns::Namespace;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut ns = Namespace::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-b" => {
                ns.bind(&args[i+1], &args[i+2]);
                i += 3;
            }
            "-u" => {
                ns.union(&args[i+1], &args[i+2]);
                i += 3;
            }
            _ => {
                break;
            }
        }
    }

    let cmd_args = &args[i..];
    if cmd_args.is_empty() {
        // basic shell
        loop {
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).unwrap() == 0 {
                break;
            }
            let input = input.trim();
            if input == "exit" {
                break;
            }
            println!("unknown command: {}", input);
        }
    } else {
        let mut cmd = Command::new(&cmd_args[0]);
        cmd.args(&cmd_args[1..]);
        let status = cmd.status().expect("failed to execute command");
        if !status.success() {
            eprintln!("command failed: {}", status);
        }
    }
}
