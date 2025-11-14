use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn test_bind() {
    let mut cmd = Command::cargo_bin("bind").unwrap();
    cmd.arg("/new").arg("/old");
    cmd.assert().success().stdout("bound '/new' to '/old'\n");
}
