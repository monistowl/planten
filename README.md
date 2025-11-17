# planten
yes really

planten is a work-in-progress Rust reimagining of Plan 9/9front: namespaces, 9P servers, and a small userland that runs on top of existing OSes today while the kernel, devices, and tooling slowly evolve.

## Setup

### Toolchain and environment
- Install the pinned Nightly toolchain from `rustup toolchain install nightly` and confirm `rust-toolchain.toml` lives at the workspace root so everyone builds against the same nightly release.
- Verify your shell has `rustup` in place, install any missing system dependencies for cross-host scripting, and reuse the same nightly channel when running `cargo fmt`, `cargo clippy`, or any build/test commands.

### Dependencies and helpers
- Run `cargo fetch` from the workspace root to populate all workspace dependencies before you start iterating.
- Ensure helper scripts under `tools/` (e.g., `tools/check_cargo_bin.sh`, `tools/plan9-qemu/*.sh`) are executable; they assume the nightly toolchain and sometimes rely on environment variables documented elsewhere.
- Run `tools/check_cargo_bin.sh` locally before committing to catch deprecated `Command::cargo_bin` usage in `userspace/planten_coreutils/tests`.

### Fixtures and golden captures
- Regenerate RAMFS golden traces with `cargo run -p capture_golden` whenever 9P semantics change; the binaries land under `tests/golden_traces` and are replayed by the RAMFS and ProcFS suites.
- Use `cargo run -p capture_procfs` or the appropriate `tools/capture_*` helper when ProcFS, NetFS, DevFS, or SrvFS behaviors evolve; each tool records deterministic request/response pairs under `tests/proc_golden` or similar directories described in `docs/pseudofs-workflow.md`.
- After updating golden data, rerun `cargo test --workspace` and the relevant integration suites so the replay helpers still match the new behavior.

### Testing and verification
- Run `cargo test --workspace` to exercise unit, integration, and golden-replay suites; some crates (`planten_fs_dev`, `planten_fs_net`, `planten_fs_srv`) include their own coverage targeting the newly captured traces.
- For quicker iteration, restrict tests to a single crate such as `cargo test -p planten_fs_ramfs --test golden_integration` or the corresponding integration test that validates a pseudo-filesystem.

## Quick start

- Start a namespace shell with `cargo run -p planten_coreutils --bin 10_ns -- -b /tmp/example /etc`; it rebuilds a namespace, binds `/etc`, drops you into an rc-like shell, and persists the mount plan to `~/.planten/ns.json`.
- Use `cargo run -p planten_coreutils --bin mount -- /tmp/fs /tmp/one /tmp/two` or `bind` to mutate the namespace that `10_ns`, `bind`, `mount`, and `nsctl` jointly manage.
- Launch pseudo-filesystem servers: RAMFS on `127.0.0.1:5640` (`cargo run -p planten_fs_ramfs --bin server`), ProcFS/NetFS/DevFS/SrvFS servers via their crate binaries, and mount them with `10_ns -p9 /mnt/<name> addr /` when probing new trees.
- Namespaces auto-mount ProcFS, NetFS, DevFS, and SrvFS through the helpers in `userspace/planten_coreutils/src/bin/10_ns.rs`, so `/proc`, `/net`, `/dev`, and `/srv` become available immediately after the namespace starts and the servers are running.

## Documentation

- [Architecture](docs/architecture.md) explains how the kernel, libs, pseudo-filesystems, and tooling compose a Plan 9-style stack.
- [Development](docs/development.md) captures the nightly toolchain, lint helpers, and golden-trace workflow for maintaining deterministic fixtures.
- [Pseudo-filesystem workflow](docs/pseudofs-workflow.md) walks through adding new `/proc`-like, `/net`, `/dev`, `/srv`, or future trees with FsServer crates, capture tooling, and namespace helpers.
- [Compatibility matrix](docs/compatibility-matrix.md) tracks which Plan 9 features are supported, how up-to-date the golden traces are, and what gaps remain.
- [Plan 9/QEMU harness](docs/plan9-qemu.md) documents the helper scripts that download a 9front/Plan 9 ISO, boot it in QEMU, and replay namespaces inside the guest.

## Current status

- `planten_ns` models bind/union/9P mounts, persists the ordered mount plan to `~/.planten/ns.json`, and auto-mounts ProcFS/NetFS/DevFS/SrvFS so the helpers always reconstruct the same namespace view.
- `planten_9p` provides framing, encoding/decoding, and client/server helpers that cover all core 9P2000 messages used by RAMFS, ProcFS, NetFS, DevFS, and SrvFS.
- `planten_fs_ramfs` exposes a threaded in-memory filesystem with full message support and golden fixtures under `tests/golden_traces`.
- `planten_fs_proc` mirrors the Plan 9 `/proc` tree (per-pid directories, `cmdline`, `status`, `stat`, `statm`, `info`, `fd`, `task`) and ships capture tooling plus replay tests to keep records deterministic.
- `planten_fs_net` serves `/net/interfaces`, `/net/tcp`, and `/net/udp` from the host stack, while `planten_fs_dev` supplies `/dev/null`, `/dev/zero`, `/dev/random`, `/dev/console`, and `planten_fs_srv` exposes `/srv/<service>/ctl`.
- Golden capture tooling for each pseudo-filesystem follows the pattern described in `docs/pseudofs-workflow.md`, so new trees adopt the same tests and documentation instead of inventing ad-hoc scripts.

## Plan 9/QEMU testing

- `tools/plan9-qemu/setup.sh` downloads the chosen ISO, verifies its SHA256, builds a qcow2 disk, and sets environment variables such as `PLAN9_ISO_URL`/`PLAN9_DISTRO` for reproducible boots.
- `tools/plan9-qemu/run.sh` boots the guest headlessly, forwards ports (e.g., host 1564→guest 564), exposes serial/shared folders, and waits for the guest to reach a known prompt.
- `tools/plan9-qemu/ci-runner.sh` wraps `run.sh`, waits for the 9P port, runs `cargo run -p plan9_qemu_client --quiet`, and halts the VM; it is the CI gate for Plan 9-based tests.
- Copy `~/.planten/ns.json` into the guest with `tools/plan9-qemu/apply-ns.sh` or automate the replay with `tools/plan9-qemu/replay-ns.sh` (requires `expect`); both scripts and their usage are covered in `docs/plan9-qemu.md`.

Namespace state is serialized to `~/.planten/ns.json` (with `/srv/planten/ns.json` as a fallback when `$HOME` is unavailable), so every helper shares the same mount order.
