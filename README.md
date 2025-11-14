# planten
yes really

planten is a work-in-progress Rust reimagining of Plan 9/9front: namespaces, 9P servers, and a small userland that runs on top of existing OSes today while the kernel, devices, and tooling slowly evolve.

Current status
--------------
- The `planten_ns` crate models bind/union/9P mounts and backs the `bind`, `mount`, `nsctl`, and `10_ns` helpers so you can compose per-process namespaces and launch binaries inside them.
- `planten_9p` now provides framing utilities plus a tiny client that can `version`, `attach`, `walk`, `open`, `read`, and `clunk` against remote services; `10_ns` uses it to probe and mount remote shares before transitioning to Linux bind mounts (covered in `userspace/planten_coreutils/src/bin/10_ns.rs`).
- `planten_fs_ramfs` hosts a threaded 9P server that exports an in-memory filesystem tree; it listens on `127.0.0.1:5640`, handles version/attach/walk/open/read/clunk, and can serve the example files that are pre-populated at startup.

Usage notes
-----------
- `cargo run -p planten_coreutils --bin 10_ns -- -b /tmp/example /etc` will build a fresh namespace, bind `/etc` under `/tmp/example`, and keep you in an rc-like shell that persists the namespace to `/tmp/ns.json`.
- Use `cargo run -p planten_coreutils --bin mount -- /tmp/fs /tmp/one /tmp/two` to add union mounts, or `cargo run -p planten_coreutils --bin bind -- /tmp/fs /tmp/one` to create simple binds.
- Start the RAMFS 9P service with `cargo run -p planten_fs_ramfs --bin server`; then run `10_ns -p9 /mnt/ramfs 127.0.0.1:5640 /` to probe and mount it (on a Linux host).

As the kernel/runtime grows, this repo will continue layering 9P first-class abstractions, but these pieces already let you play with namespaces and remote Plan 9 file servers locally.
