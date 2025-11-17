# Architecture

This document captures the high-level structure of the planten workspace so contributors know
where the Plan 9/9front semantics live and how the components interact.

## Workspace structure

- `kernel/planten_kernel` is where the minimal Rust kernel, scheduler, and core abstractions will
  live; the roadmap outlines a small trusted core with unsafe confined to HAL/driver modules.
- `libs/` hosts shared 9P, filesystem, namespace, and device traits. `planten_9p` defines framing
  and encoding helpers plus a client/server pair, while `planten_ns` owns per-process bind/union
  logic and persistence helpers. `planten_fs_ramfs`, `planten_fs_proc`, and future filesystem crates
  implement `FsServer`-like traits that talk 9P to serve `/srv`, `/proc`, and related namespaces.
- `userspace/planten_coreutils` packages the core utilities such as `bind`, `mount`, `nsctl`, `10_ns`,
  and the rc-like shell that persists namespaces.
- `tools/capture_golden` and `tests/` provide tooling for compatibility verification and golden
  fixtures for the RAMFS 9P interactions.

## Namespaces and 9P

The namespace helpers (`bind`, `mount`, `10_ns`, `nsctl`) all read/write the ordered mount plan
persisted to `~/.planten/ns.json` (with `/srv/planten/ns.json` as fallback). When a helper runs, it
rebuilds the same plan and replays the union/bind operations so every process can reconstruct its
view deterministically. This JSON-backed strategy keeps namespaces portable, lets `10_ns` merge
sequential unions, and prevents the command-line tools from stomping on each other.

`planten_9p` centralizes message framing, encoding/decoding for version/auth/attach/walk/open/...,
and defines the `RawMessage` helpers used by both clients and the RAMFS server. That crate is the
shared protocol layer between the kernel, libs, and userland. `planten_fs_ramfs` exposes a threaded
9P server (listening on `127.0.0.1:5640`) and implements all standard requests: reads, writes,
stat, twstat, remove, clone, flush, and error handling. `tools/capture_golden` drives the same
server programmatically and stores golden frames under `tests/golden_traces`. `planten_fs_net` mirrors
the host networking stack by serving `/net/interfaces`, `/net/tcp`, and `/net/udp`, sourcing data
from `/sys/class/net` and `/proc/net` so `/net` becomes another FsServer-backed tree alongside
RAMFS and ProcFS. `planten_fs_dev` exposes `/dev/null`, `/dev/zero`, `/dev/random`, and `/dev/console`
so namespaces can interact with those classic devices via 9P, and `docs/pseudofs-workflow.md` details
how to capture and replay traces for these pseudo-filesystems. `planten_fs_srv` mirrors `/srv` by listing
service directories and serving a `ctl` file per entry, giving namespaces a consistent service mount
path that can point at local or remote servers via the same 9P interface.

## Userspace and tooling

The Rust-based coreutils live under `userspace/planten_coreutils`; they consume `planten_ns` so
commands like `mount -- /tmp/fs /tmp/one /tmp/two` and the `10_ns` shell can bind/union and then
launch child processes inside the constructed namespace. Namespaces immediately serialize their
state so subsequent helpers and shells use the same mount ordering.

Testing relies on golden traces captured from the RAMFS server—`tests/golden_traces` stores the
sequence of request/response frames produced by the validated server, and `tests/proc_client` and
other suites replay those sequences against the server and assert on the recorded responses.

Developers regenerate fixtures with `cargo run -p capture_golden` whenever the protocol behavior
changes; `docs/development.md` documents that workflow alongside the nightly toolchain and linting
guardrails.

## ProcFS tree and golden capture

- `libs/planten_fs_proc` now exposes a `/proc` tree whose per-pid directories mirror the Plan 9 layout:
  `cmdline`, `status`, `stat`, `statm`, `info`, `mounts`, `fd/`, and `task/self` all round-trip through
  the same `FsServer` trait used by RAMFS. Each directory listing and file stat incorporates real-time
  data from `sysinfo`, while placeholder `fd`/`task` entries keep the tree navigable when the host
  provides fewer details.
- The golden trace workflow now includes a ProcFS recorder (`tools/capture_procfs`). It spins up the
  ProcFS server, issues a fixed sequence of handshake, root, and per-pid operations, and dumps the
  resulting request/response pairs under `tests/proc_golden`. The new `libs/planten_fs_proc/tests/proc_golden_integration.rs`
  test plays back the handshake/root sequence and ensures the recorded response types still match the
  live server, making it easy to spot regressions when adding new proc entries or changing data
  formatting.
