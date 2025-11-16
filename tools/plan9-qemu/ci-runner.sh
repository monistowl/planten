#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
RUNNER="$SCRIPT_DIR/run.sh"
BASE_DIR="${PLAN9_BASE_DIR:-$SCRIPT_DIR/.plan9}"
PORT="${PLAN9_QEMU_ADDR:-127.0.0.1:1564}"

if [[ ! -f "$BASE_DIR/plan9.qcow2" ]]; then
  echo "Disk image missing. Run tools/plan9-qemu/setup.sh and install Plan 9 before running this harness."
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required to poll the guest port."
  exit 1
fi

QEMU_LOG="$BASE_DIR/plan9-qemu.log"

echo "Starting background Plan 9 QEMU (output -> $QEMU_LOG)..."
$RUNNER -- --nographic -monitor none -serial mon:stdio > "$QEMU_LOG" 2>&1 &
QEMU_PID=$!

cleanup() {
  echo "Tearing down QEMU (pid $QEMU_PID)..."
  kill "$QEMU_PID" 2>/dev/null || true
  wait "$QEMU_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "Waiting for Plan 9 guest to accept 9P on $PORT..."
for _ in {1..60}; do
  if python3 - <<PY >/dev/null 2>&1
import socket, os, sys
host, port = "$PORT".split(":")
with socket.create_connection((host, int(port)), timeout=1): pass
PY
  then
    echo "Port $PORT is open."
    break
  fi
  sleep 1
done

echo "Running `plan9_qemu_client` handshake..."
cargo run -p plan9_qemu_client --bin plan9_qemu_client --quiet

echo "Happy handshake. See $QEMU_LOG for QEMU console output."
