# Compatibility Matrix

This document tracks where planten currently stands compared to the corresponding
9front features and where we still have work to do.

| Feature | 9front Status | planten Status | Notes |
|---|---|---|---|
| 9P2000 message set (version/auth/attach/walk/open/create/read/write/...) | Supported | Implemented | `planten_9p` provides framing + client/server helpers used by RAMFS and coreutils. |
| Namespace bind/union helpers and persistence | Stable | Implemented | `bind`, `mount`, `nsctl`, and `10_ns` all share the JSON-backed mount plan saved at `~/.planten/ns.json`. |
| RAMFS 9P server (stat/read/write/remove/clone/twstat/flush) | Supported | Implemented | `planten_fs_ramfs` exposes a threaded server on `127.0.0.1:5640`; tests/golden_traces cover request/response sequences. |
| `/proc`-like 9P filesystem | Supported | Planned | There is scope to mirror Plan 9 `/proc` layout; see bd task `planten-dh9` and `libs/planten_fs_proc`. |
| Golden trace capture & regression tests | N/A | Implemented | `tools/capture_golden` regenerates `tests/golden_traces`; suites replay them via `tests/proc_client` and other integration tests. |
