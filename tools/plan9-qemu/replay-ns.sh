#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BASE_DIR="${PLAN9_BASE_DIR:-$SCRIPT_DIR/.plan9}"
SHARE_DIR="${PLAN9_NS_SHARE_DIR:-$BASE_DIR/ns-share}"
NS_FILE="${PLAN9_NS_FILE:-${HOME:-/root}/.planten/ns.json}"
QEMU_LOG="$BASE_DIR/plan9-replay.log"
SERIAL_PORT="${PLAN9_QEMU_SERIAL_TELNET_PORT:-4000}"
USER="${PLAN9_GUEST_USER:-sys}"
PASS="${PLAN9_GUEST_PASS:-sys}"

if [[ ! -f "$NS_FILE" ]]; then
  echo "Namespace file $NS_FILE not found. Run bind/mount helpers to create it."
  exit 1
fi

mkdir -p "$SHARE_DIR"
cp "$NS_FILE" "$SHARE_DIR/ns.json"

expect <<EOF
spawn "$SCRIPT_DIR/run.sh" -- -display none -serial "telnet:127.0.0.1:${SERIAL_PORT},server,nowait" -monitor none
set timeout 600
expect {
  -re "login: $" {
    send "$USER\r"
    exp_continue
  }
  -re "Password: $" {
    send "$PASS\r"
    exp_continue
  }
  -re "% $" { }
}

proc send_cmd {cmd} {
  expect -re "% $"
  send "$cmd\r"
}

send_cmd "mount -t 9p hostshare /n/host"
send_cmd "ls /n/host"
send_cmd "cp /n/host/ns.json /tmp/ns.json"
send_cmd "cat /tmp/ns.json | head"
send_cmd "nsctl"
send_cmd "fsys main sync"
expect -re "% $"
send "fsys main halt\r"
expect eof
EOF

echo "Replay completed. See $QEMU_LOG for console output."
