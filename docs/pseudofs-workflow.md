# Pseudo-filesystem workflow

Implementing a new pseudo-filesystem (e.g., `/proc`, `/net`, `/dev`, `/srv`) follows the same pattern so we stay compatible with the roadmap and testing infrastructure:

1. **Design the tree** – decide which entries the directory should expose, whether they are files or further directories (e.g., `/proc/<pid>/stat`, `/net/interfaces`). Map each entry to either host data (e.g., `/proc/net/tcp`) or synthesized details.
2. **Implement `FsServer`** – add a crate such as `planten_fs_proc` or `planten_fs_net` that implements `FsServer`. Offer `walk`, `open`, `read`, and `stat` so callers can traverse the layout without needing special-case logic.
3. **Expose a runtime server** – provide a `server` helper that listens on a TCP address and drives the `FsServer`. The existing ProcFS server shows how to let `10_ns` or a dedicated binary start it, and `tools/capture_procfs` proves how to reuse the server in automation.
4. **Capture golden traces** – create a capture tool under `tools/` that bootstraps the server, runs a deterministic sequence of 9P requests, and writes both requests and responses to `tests/<pseudo>/` so you can replay them later.
5. **Write golden regression tests** – add an integration test (like `libs/planten_fs_proc/tests/proc_golden_integration.rs`) that replays the recorded frame pairs, comparing message types/bodies so we notice any change in behavior.
6. **Integrate with namespaces** – in `planten_ns` and the helpers (`10_ns`, `bind`, `mount`), decide whether to automatically mount the pseudo-filesystem. ProcFS is already auto-mounted; apply the same logic when the `/net`, `/dev`, or `/srv` servers are ready.
7. **Document and verify** – mention the new filesystem and capture/test steps in `docs/architecture.md` or a new subsection so future contributors replicate the workflow instead of inventing ad-hoc scripts.

Following these steps earns a consistent pseudo-filesystem rollout and keeps the golden regression suite and documentation in sync.
