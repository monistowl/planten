# Compatibility Matrix

This document tracks where planten currently stands compared to the corresponding
9front features and where we still have work to do.

| Feature | 9front Status | planten Status | Notes |
|---|---|---|---|
| 9P2000 message set (version/auth/attach/walk/open/create/read/write/...) | Supported | Implemented | `planten_9p` provides framing + client/server helpers used by RAMFS, ProcFS, NetFS, DevFS, and SrvFS. |
| Namespace bind/union helpers and persistence | Stable | Implemented | `bind`, `mount`, `nsctl`, and `10_ns` all share the JSON-backed mount plan saved at `~/.planten/ns.json` and auto-mount the pseudo-filesystems described in `docs/pseudofs-workflow.md`. |
| RAMFS 9P server (stat/read/write/remove/clone/twstat/flush) | Supported | Implemented | `planten_fs_ramfs` exposes a threaded server on `127.0.0.1:5640`; tests/golden_traces cover request/response sequences. |
| `/proc`-like 9P filesystem | Supported | Implemented | `planten_fs_proc` mirrors the Plan 9 `/proc` layout with per-pid directories, `cmdline`, `stat`, `status`, `fd`, and `task`; capture tooling records deterministic traces under `tests/proc_golden`. |
| `/net` pseudo-filesystem | Supported | Implemented | `planten_fs_net` serves `/net/interfaces`, `/net/tcp`, `/net/udp` from the host stack, and golden tests ensure deterministic behavior. |
| `/dev` pseudo-filesystem | Supported | Implemented | `planten_fs_dev` exposes `/dev/null`, `/dev/zero`, `/dev/random`, `/dev/console` via a 9P server; capture/replay tests guard against regressions. |
| `/srv` pseudo-filesystem | Supported | Implemented | `planten_fs_srv` lists `/srv/<service>/ctl`, runs its own server, and auto-mount helpers (e.g., `ns.ensure_srvfs()`) keep the interface predictable. |
| Golden trace capture & regression tests | N/A | Implemented | `tools/capture_golden`, `tools/capture_procfs`, and the other `tools/capture_*` helpers regenerate the fixtures replayed by integration suites. |
