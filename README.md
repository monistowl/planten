# planten
yes really

planten is a work-in-progress Rust reimagining of Plan 9/9front: namespaces, 9P servers, and a small userland that runs on top of existing OSes today while the kernel, devices, and tooling slowly evolve.

## Setup

1. **Install the pinned toolchain**: `rustup toolchain install nightly` and verify `rust-toolchain.toml` exists at the repo root; most scripts assume the same nightly release.
2. **Fetch tooling**: run `cargo fetch` from the workspace root to populate dependencies before you start iterating.
3. **Ensure helper scripts are executable** (e.g., `tools/check_cargo_bin.sh` and any `tools/plan9-qemu/*.sh`). They rely on the same nightly Rust toolchain, so run `cargo fmt`/`cargo clippy` after toolchain setup.
4. **Generate fixtures**: golden-trace data lives in `tests/golden_traces`; regenerate them via `cargo run -p capture_golden` whenever 9P semantics change.
5. **Verify locally**: Run `cargo test -p planten_fs_ramfs --test golden_integration` plus `cargo test --workspace` to exercise the namespace and filesystem suites.

## CLI helpers

- `cargo run -p planten_coreutils --bin 10_ns -- -b /tmp/example /etc` builds a fresh namespace and drops you into an rc-like shell that persists state to `~/.planten/ns.json`.
- `cargo run -p planten_coreutils --bin bind`/`mount`/`nsctl` mutate the namespace that `10_ns` and the shell share.
- The RAMFS server listens on `127.0.0.1:5640` (`cargo run -p planten_fs_ramfs --bin server`) and works with `10_ns -p9 /mnt/ramfs 127.0.0.1:5640 /`.

## Documentation

- [Architecture](docs/architecture.md) describes how `libs/`, `kernel/`, `userspace/`, and tooling interlock.
- [Development](docs/development.md) covers the nightly toolchain, lint helpers, and golden-trace workflow.
- [Plan 9/QEMU harness](docs/plan9-qemu.md) explains the QEMU scripts, distros, and CI runner.
- [Compatibility matrix](docs/compatibility-matrix.md) lists supported targets and known gaps.

## Plan 9/QEMU testing

- `tools/plan9-qemu/setup.sh` downloads a 9front (or Plan 9) ISO, verifies its SHA256, and prepares a qcow2 disk.
- `tools/plan9-qemu/run.sh` boots headlessly, forwards port `564→1564`, and exposes `serial`/`plan9share` helpers for the client.
- `tools/plan9-qemu/ci-runner.sh` waits for the guest, runs `cargo run -p plan9_qemu_client --quiet`, and halts the VM for CI validation.
- Copy `~/.planten/ns.json` into the guest via `apply-ns.sh`/`replay-ns.sh` to replay recorded namespaces inside the VM.

Current status
--------------
- The `planten_ns` crate models bind/union/9P mounts and backs the `bind`, `mount`, `nsctl`, and `10_ns` helpers so you can compose per-process namespaces and launch binaries inside them.
- `planten_9p` now provides framing utilities plus a client that can handle all standard 9P2000 messages: `version`, `auth`, `attach`, `walk`, `open`, `create`, `read`, `write`, `clunk`, `stat`, `wstat`, `remove`, and `flush`.
- `planten_fs_ramfs` hosts a threaded 9P server that exports an in-memory filesystem tree; it listens on `127.0.0.1:5640` and handles all messages supported by the client, including creating, writing, removing, and stat-ing files.

Usage notes
-----------
- `cargo run -p planten_coreutils --bin 10_ns -- -b /tmp/example /etc` will build a fresh namespace, bind `/etc` under `/tmp/example`, and keep you in an rc-like shell that persists the namespace to `/tmp/ns.json`.
- Use `cargo run -p planten_coreutils --bin mount -- /tmp/fs /tmp/one /tmp/two` to add union mounts, or `cargo run -p planten_coreutils --bin bind -- /tmp/fs /tmp/one` to create simple binds.
- Start the RAMFS 9P service with `cargo run -p planten_fs_ramfs --bin server`; then run `10_ns -p9 /mnt/ramfs 127.0.0.1:5640 /` to probe and mount it (on a Linux host).

Plan 9/QEMU harness
--------------------
- See `docs/plan9-qemu.md` for the new `tools/plan9-qemu/{setup,run}.sh` helpers that download a known 9front ISO, boot it under QEMU, and expose forwarded ports (host 1564→guest 564, 1567→567, etc.) so we can validate planten 9P clients against a real Plan 9 guest.
- The new `tools/plan9-qemu/ci-runner.sh` script wraps `run.sh`, waits for the guest’s 9P port, runs `cargo run -p plan9_qemu_client --quiet`, and cleans up the VM; use it as the first gate in any Plan 9 QEMU-based CI job.
- Pass `PLAN9_DISTRO=plan9` (or override `PLAN9_ISO_URL`/`PLAN9_ISO_SHA256`) if you’d rather test against vanilla Plan 9 instead of 9front.
- Run `tools/plan9-qemu/apply-ns.sh` after generating `~/.planten/ns.json`; it shares the namespace JSON into the guest so you can replay it via `10_ns`/`nsctl` inside the actual 9front instance.
- Use `tools/plan9-qemu/replay-ns.sh` to automate the guest-side replay: it starts QEMU with the shared namespace JSON, logs in via `expect`, mounts the shared directory, copies the JSON, runs `nsctl`, and shuts down the guest. Ensure `expect` is installed before invoking the script (the CI workflow already installs it).

Namespace state is now serialized to `~/.planten/ns.json` (with `/srv/planten/ns.json` as a fallback when `$HOME` isn’t available), so each `10_ns`, `bind`, `mount`, and `nsctl` invocation reads/writes the same ordered mount list.

Linting
-------
- Run `tools/check_cargo_bin.sh` before committing so we catch any accidental reintroduction of `Command::cargo_bin` (the script fails if that deprecated helper appears in `userspace/planten_coreutils/tests`).

Namespace ordering
------------------
- `10_ns` now aggregates each recorded `MountEntry` into an explicit `mount_plan()`, merging sequential union entries targeting the same mount point so we preserve insertion order while still letting unions grow. The same plan is replayed when the child namespace is constructed, and the saved JSON is the single source of truth for all CLI helpers.

As the kernel/runtime grows, this repo will continue layering 9P first-class abstractions, but these pieces already let you play with namespaces and remote Plan 9 file servers locally.
