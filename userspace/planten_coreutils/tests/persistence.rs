#![allow(deprecated)]

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use planten_ns::Namespace;
use predicates::prelude::*;
use std::env;
use tempfile::tempdir;

fn set_home(tmp: &tempfile::TempDir) {
    unsafe { env::set_var("HOME", tmp.path()) };
}

#[test]
fn nsctl_reads_saved_namespace_state() {
    let tmp = tempdir().unwrap();
    set_home(&tmp);

    let mut ns = Namespace::new();
    ns.bind("/from", "/old");
    ns.union("/from", "/first");
    ns.save_to_storage().unwrap();

    let mut cmd = Command::new(cargo_bin!("nsctl"));
    cmd.env("HOME", tmp.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("/from -> (bind)"));
}
