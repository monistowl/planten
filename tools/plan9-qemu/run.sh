#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BASE_DIR="${PLAN9_BASE_DIR:-$SCRIPT_DIR/.plan9}"
ISO_PATH="${PLAN9_ISO_PATH:-$BASE_DIR/9front-10931.386.iso}"
IMAGE_PATH="${PLAN9_IMAGE:-$BASE_DIR/plan9.qcow2}"

QEMU_BIN="${PLAN9_QEMU_BIN:-qemu-system-x86_64}"
MEM="${PLAN9_QEMU_MEM:-2048}"
CORES="${PLAN9_QEMU_SMP:-2}"
VGA="${PLAN9_QEMU_VGA:-std}"
ACCEL="${PLAN9_QEMU_ACCEL:-kvm:tcg}"
NET_FORWARD="${PLAN9_QEMU_NET_FORWARD:-hostfwd=tcp::1564-:564,hostfwd=tcp::1567-:567,hostfwd=tcp::1570-:17010}"
DISPLAY="${PLAN9_QEMU_DISPLAY:-default}"
SERIAL="${PLAN9_QEMU_SERIAL:-mon:stdio}"

MODE="run"
EXTRA_ARGS=()

function usage() {
    cat <<EOF
Usage: $(basename "$0") [--install] [-- <extra qemu args>]

--install        Include the ISO as a CD-ROM and boot the installer (uses ISO_PATH)
--help           Show this help text

Environment variables:
  PLAN9_BASE_DIR        Directory created by setup.sh (defaults to tools/plan9-qemu/.plan9)
  PLAN9_IMAGE           Disk image path (default $BASE_DIR/plan9.qcow2)
  PLAN9_ISO_PATH        Decompressed ISO path (default $BASE_DIR/9front-10931.386.iso)
  PLAN9_QEMU_BIN        QEMU binary (default qemu-system-x86_64)
  PLAN9_QEMU_MEM        RAM in megabytes (default $MEM)
  PLAN9_QEMU_SMP        Number of CPU cores (default $CORES)
  PLAN9_QEMU_VGA        VGA device (default $VGA)
  PLAN9_QEMU_ACCEL      Acceleration string (default $ACCEL)
  PLAN9_QEMU_NET_FORWARD Comma-separated guest port forwards (default $NET_FORWARD)
  PLAN9_QEMU_DISPLAY    Display backend (default $DISPLAY)
  PLAN9_QEMU_SERIAL     Serial device (default $SERIAL)
  PLAN9_QEMU_SHARED_DIR Expose a host directory as a 9P share
EOF
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --install)
            MODE="install"
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --)
            shift
            EXTRA_ARGS=("$@")
            break
            ;;
        *)
            EXTRA_ARGS+=("$1")
            shift
            ;;
    esac
done

if ! command -v "$QEMU_BIN" >/dev/null 2>&1; then
    echo "QEMU not found: $QEMU_BIN" >&2
    exit 1
fi

if [[ ! -f "$IMAGE_PATH" ]]; then
    echo "Disk image $IMAGE_PATH is missing. Run tools/plan9-qemu/setup.sh first." >&2
    exit 1
fi

if [[ "$MODE" == "install" ]]; then
    if [[ ! -f "$ISO_PATH" ]]; then
        echo "ISO $ISO_PATH is missing. Run tools/plan9-qemu/setup.sh before --install." >&2
        exit 1
    fi
fi

QEMU_ARGS=(
    "-machine" "q35,accel=$ACCEL"
    "-smp" "$CORES"
    "-m" "$MEM"
    "-vga" "$VGA"
    "-serial" "$SERIAL"
    "-net" "nic"
    "-net" "user,$NET_FORWARD"
    "-drive" "file=$IMAGE_PATH,if=virtio,cache=writeback"
)

if [[ "$DISPLAY" != "default" ]]; then
    QEMU_ARGS+=("-display" "$DISPLAY")
fi

if [[ -n "${PLAN9_QEMU_SHARED_DIR:-}" ]]; then
    QEMU_ARGS+=("-virtfs" "local,path=$PLAN9_QEMU_SHARED_DIR,mount_tag=hostshare,security_model=passthrough,id=hostshare")
fi

if [[ "$MODE" == "install" ]]; then
    QEMU_ARGS+=("-cdrom" "$ISO_PATH" "-boot" "order=d")
else
    QEMU_ARGS+=("-boot" "order=c")
fi

QEMU_ARGS+=("${EXTRA_ARGS[@]}")

echo "Running Plan 9 QEMU ($MODE mode): $QEMU_BIN ${QEMU_ARGS[*]}"
exec "$QEMU_BIN" "${QEMU_ARGS[@]}"
