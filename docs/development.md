# Development

## Toolchain & build

- The workspace pins `nightly` Rust via `rust-toolchain.toml`, so install the same channel before running `cargo` commands.
- Use `cargo build --workspace` (or target-specific crates like `cargo build -p planten_fs_ramfs`) to compile the kernel, libs, and userland together. Cargo will pick up the workspace members listed in `Cargo.toml`.
- For incremental experimentation, `cargo run -p planten_coreutils --bin 10_ns -- -b /tmp/example /etc` and the other coreutils binaries live under `userspace/planten_coreutils`.

## Testing & linting

- Run `cargo test --workspace` to execute unit and integration suites; some modules such as `tools/capture_golden` produce assets used by golden tests.
- The helper script `tools/check_cargo_bin.sh` scans `userspace/planten_coreutils/tests` for the deprecated `Command::cargo_bin` helper and fails if it finds any occurrences. Include this script in your pre-commit flow to keep the new `cargo_bin!` macros enforced.
- Watch out for the golden fixtures under `tests/golden_traces`; updating the 9P message flow usually requires regenerating them before the tests pass again.

## Golden trace regeneration

- Regenerate the golden request/response sequences after any change that affects the RAMFS 9P protocol by running `cargo run -p capture_golden`.
- `tools/capture_golden` starts an in-memory RAMFS server (`libs/planten_fs_ramfs::server`) and captures each request/response pair, writing them to `tests/golden_traces/*.bin`. Commit the updated binaries together with code changes so `tests/proc_client` and other suites stay in sync.
- Pseudo-filesystems such as ProcFS, NetFS, DevFS, and SrvFS follow the same capture pattern; run the corresponding `tools/capture_*` helper (e.g., `cargo run -p capture_procfs`) so `tests/proc_golden` and similar fixtures stay aligned with the live servers described in `docs/pseudofs-workflow.md`.
- When golden traces change, rerun `cargo test --workspace` to make sure the replayed sequences still match the new outputs and there are no regressions.
