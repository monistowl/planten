#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BASE_DIR="${PLAN9_BASE_DIR:-$SCRIPT_DIR/.plan9}"
NS_FILE="${PLAN9_NS_FILE:-${HOME:-/root}/.planten/ns.json}"
SHARE_DIR="${PLAN9_NS_SHARE_DIR:-$BASE_DIR/ns-share}"

if [[ ! -f "$NS_FILE" ]]; then
  echo "Namespace file $NS_FILE not found. Run bind/mount helpers to create it."
  exit 1
fi

mkdir -p "$SHARE_DIR"
cp "$NS_FILE" "$SHARE_DIR/ns.json"
echo "Copied namespace definition to share at $SHARE_DIR/ns.json"

echo "Starting QEMU with shared ns.json..."
PLAN9_QEMU_SHARED_DIR="$SHARE_DIR" PLAN9_QEMU_ADDR="${PLAN9_QEMU_ADDR:-127.0.0.1:1564}" "$SCRIPT_DIR/run.sh" "${@}"

cat <<'EOF'
Inside the guest:
  1. Mount the host share: `mount -t 9p hostshare /n/host`
  2. Copy the ns file into the guest: `cp /n/host/ns.json /tmp/ns.json`
  3. Run your namespace replay helper (10_ns or similar) pointing at /tmp/ns.json log:
       `10_ns -p9 /tmp ns.json ???` (adjust command to apply the recorded mount plan).
  4. Confirm `/` shows the same mounts (`nsctl`), then run `fsys main sync; fsys main halt`.
EOF
