#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn test_mount_bind() {
    let mut cmd = Command::new(cargo_bin!("mount"));
    cmd.arg("/new").arg("/old");
    cmd.assert().success().stdout("bound '/new' to '/old'\n");
}

#[test]
fn test_mount_union() {
    let mut cmd = Command::new(cargo_bin!("mount"));
    cmd.arg("/new").arg("/old1").arg("/old2");
    cmd.assert()
        .success()
        .stdout("created union mount at '/new'\n");
}
