# Plan 9 on QEMU

This guide implements the plan captured in bd issue `planten-bsd`: we source a recent 9front ISO, spin up Plan 9/9front under QEMU, and exercise the planten 9P suites against that guest. The instructions below mirror the workflow described in the Plan 9 QEMU wiki (https://9p.io/wiki/plan9/Installing_Plan_9_on_Qemu/index.html).

## Decision summary

1. **ISO Source** – By default we pin to the 9front 10931 ISO archive hosted at `http://iso.only9fans.com/release/9front-10931.386.iso.gz`, but you can set `PLAN9_DISTRO=plan9` (or `PLAN9_ISO_URL`/`PLAN9_ISO_NAME`) to use a vanilla Plan 9 ISO instead. The script auto-detects `.gz` downloads and decompresses them; plan9 binaries are served uncompressed so set `PLAN9_ISO_SHA256` manually if you know the checksum.
2. **Disk image** – The script creates a `qcow2` image named `plan9.qcow2` under `tools/plan9-qemu/.plan9/`. QCOW2 balances iteration speed with snapshot capability, and the install path lets folks recreate the image from scratch when required.  
3. **Networking & automation** – `tools/plan9-qemu/run.sh` uses `-net user` with host-forwards (`host 1564→guest 564`, `1567→567`, `1570→17010`) so host utilities can open Plan 9 9P ports without needing root or bridges. The same script can boot the installer (`--install`) or the final disk image and exposes `PLAN9_QEMU_SHARED_DIR` for mounting host directories via 9P/`virtfs`.  
4. **Test harness scope** – The initial implementation just delivers repeatable setup/run helpers; future tests can invoke `run.sh` from a CI job, wait until the guest finishes booting, copy binaries via the forwarded 9P port or a shared directory, run regression commands, and then halt using `fsys main sync`/`halt` as recommended by the wiki.

## Running the helper scripts

1. **Set up the environment:**  
   ```bash
   tools/plan9-qemu/setup.sh
   ```  
   The script downloads and verifies the archive, decompresses the ISO, and creates the `qcow2` disk. Override variables such as `PLAN9_BASE_DIR`, `PLAN9_IMAGE`, `PLAN9_DISK_SIZE`, `PLAN9_ISO_URL`, or `PLAN9_ISO_SHA256` in the environment to point to other releases or alternate layouts.

2. **Boot Plan 9 (installer or disk):**  
   ```bash
   tools/plan9-qemu/run.sh --install  # first run, boots the ISO
   tools/plan9-qemu/run.sh            # subsequent runs, boots the installed disk
   ```  
   The script accepts `--install` to boot the ISO and `--` to forward extra QEMU arguments (`-nographic`, custom `-device`, etc.). Control the CPU/memory, VGA backend, acceleration, and forwarded ports through the `PLAN9_QEMU_*` environment variables documented in the script header.  
   If the guest should service a shared host tree, export `PLAN9_QEMU_SHARED_DIR=/path/to/tree` before invoking the runner.

## Host ↔ guest connectivity notes

- The default forwards expose guest port **564** (Plan 9 9P) on host **1564**, port **567** (factotum) on **1567**, and the Plan 9 **17010** service on **1570**. Adjust `PLAN9_QEMU_NET_FORWARD` to add more port mappings from the wiki if needed.
- NAT mode limits raw ICMP, so use TCP-based interactions (9P requests, `rget`, `cpu` commands) when validating connectivity, just as the Plan 9 docs warn.
- For automated runs, consider copying mission-critical binaries into the guest via the shared-virtfs (`mount -t 9p hostshare /n/host`) instead of relying on slow serial logins.
 - Use the new `plan9_qemu_client` helper (`cargo run -p plan9_qemu_client --bin plan9_qemu_client`) to verify the forwarded 9P service: it connects to `PLAN9_QEMU_ADDR` (default `127.0.0.1:1564`), performs `version`/`attach`, reads the root directory, and exits cleanly. This binary is the starting point for any CI job that needs to assert the guest is up before running more complex workloads.

## Integrating into tests

1. Launch `run.sh` from a CI job or local helper and wait for the Plan 9 shell prompt.  
2. Use forwarded ports (e.g., host `localhost:1564`) to run Pflanzen 9P clients against the guest and compare their response with `tests/golden_traces`.  
3. After exercising the guest, shyly run `fsys main sync` followed by `fsys main halt` before exiting QEMU to keep the disk clean (this mirrors the wiki instructions for shutting down).  
4. If the harness becomes heavy for every run, keep the scripts but only trigger them for nightly/compatibility builds; the rest of the suite can remain the unit/golden tests we already have.

## Automated handshake runner

- `tools/plan9-qemu/ci-runner.sh` starts QEMU headlessly via `run.sh`, waits for the forwarded 9P port (default `127.0.0.1:1564`) to accept connections, runs `cargo run -p plan9_qemu_client --quiet` to confirm the guest is responding, and then tears down the VM. This script is the core of the “Plan 9 guest is ready” check you can consume from CI before running further integration tests.  
- Before invoking this runner the first time, run `tools/plan9-qemu/setup.sh` and manually finish installing Plan 9 so the `plan9.qcow2` image contains a bootable system that can reach the prompt without the installer. If you stop the script early, it kills QEMU, so run `fsys main sync; fsys main halt` inside the guest manually before rerunning.

## CI caching

- GitHub Actions now caches `tools/plan9-qemu/.plan9` keyed on the 9front 10931 release (`.github/workflows/plan9-qemu.yml`). The cache holds the downloaded QCOW2 image so repeated CI runs reuse the saved 9front snapshot rather than reinstalling from ISO.  
- When the release bumps, update the cache key and the `PLAN9_PREBUILT_IMAGE_URL`/`SHA256` settings in the workflow and setup script to point at the new artifact so the cache stays valid. You can force a cache reset anytime by deleting the runner’s cache entry (or updating the key string).

## Next steps / maintenance

- Cache the prepared disk image (or a snapshot) so CI doesn’t need to re-run the installer on every run.  
- Update `PLAN9_ISO_URL` / `PLAN9_ISO_SHA256` when a new 9front release lands; rerun `setup.sh` to refresh `plan9.qcow2`.  
- Extend this doc with instructions on how to mount the host `planten` repo inside the guest (e.g., `mount -t 9p hostshare /n/planten`) and how to script 9P command sequences captured from `tools/capture_golden`.  
- Track future automation/CI gating work in bd issue `planten-bsd` so changes stay linked to the original plan.
