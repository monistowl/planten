#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn test_bind() {
    let mut cmd = Command::new(cargo_bin!("bind"));
    cmd.arg("/new").arg("/old");
    cmd.assert().success().stdout("bound '/new' to '/old'\n");
}
