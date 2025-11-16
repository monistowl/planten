#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BASE_DIR="${PLAN9_BASE_DIR:-$SCRIPT_DIR/.plan9}"
PLAN9_DISTRO="${PLAN9_DISTRO:-9front}"
DISK_SIZE="${PLAN9_DISK_SIZE:-4G}"
USE_PREBUILT="${PLAN9_USE_PREBUILT_IMAGE:-true}"

declare -A DISTROS
DISTROS[9front]="http://iso.only9fans.com/release/9front-10931.386.iso.gz"
DISTROS[plan9]="https://9p.io/sys/plan9/plan9.iso"

if [[ -z "${DISTROS[$PLAN9_DISTRO]:-}" ]]; then
  echo "Unknown Plan 9 distro '$PLAN9_DISTRO'." >&2
  exit 1
fi

ISO_URL="${PLAN9_ISO_URL:-${DISTROS[$PLAN9_DISTRO]}}"
ISO_NAME="${PLAN9_ISO_NAME:-$(basename "$ISO_URL")}"
ISO_DOWNLOAD_PATH="$BASE_DIR/$ISO_NAME"
NEEDS_DECOMPRESS=false
case "$ISO_NAME" in
  *.gz) NEEDS_DECOMPRESS=true ;;
esac

ISO_PATH="${PLAN9_ISO_PATH:-$ISO_DOWNLOAD_PATH}"
if [[ "$NEEDS_DECOMPRESS" == true ]]; then
  ISO_PATH="${PLAN9_ISO_PATH:-${ISO_DOWNLOAD_PATH%.gz}}"
fi

ISO_SHA="${PLAN9_ISO_SHA256:-${PLAN9_DISTRO_SHA256:-}}"
if [[ -z "$ISO_SHA" && "$PLAN9_DISTRO" == "9front" ]]; then
  ISO_SHA="6a3228b26726843e8f0f0367499aa9a2371e7877165b39f8a1f1313312e7a6cc"
fi

IMAGE_PATH="${PLAN9_IMAGE:-$BASE_DIR/plan9.qcow2}"
PREBUILT_URL="${PLAN9_PREBUILT_IMAGE_URL:-http://iso.only9fans.com/release/9front-10931.amd64.qcow2.gz}"
PREBUILT_ARCHIVE="$BASE_DIR/$(basename "$PREBUILT_URL")"
PREBUILT_IMAGE_PATH="${PLAN9_PREBUILT_IMAGE_PATH:-$BASE_DIR/plan9-prebuilt.qcow2}"
PREBUILT_SHA="${PLAN9_PREBUILT_IMAGE_SHA256:-}"

mkdir -p "$BASE_DIR"

echo "Setting up Plan 9 environment under $BASE_DIR"

if [[ "$USE_PREBUILT" == "true" ]]; then
  if [[ ! -f "$PREBUILT_ARCHIVE" ]]; then
    echo "Downloading prebuilt Plan 9 image from $PREBUILT_URL..."
    curl -L --retry 3 --retry-delay 5 --output "$PREBUILT_ARCHIVE" "$PREBUILT_URL"
  else
    echo "Prebuilt archive already at $PREBUILT_ARCHIVE"
  fi

  if [[ -n "$PREBUILT_SHA" ]]; then
    printf "%s  %s\n" "$PREBUILT_SHA" "$PREBUILT_ARCHIVE" | sha256sum -c -
  fi

  if [[ ! -f "$PREBUILT_IMAGE_PATH" ]]; then
    echo "Decompressing prebuilt image to $PREBUILT_IMAGE_PATH..."
    gzip --decompress --keep --force "$PREBUILT_ARCHIVE"
    DECOMPRESSED="${PREBUILT_ARCHIVE%.gz}"
    if [[ "$DECOMPRESSED" != "$PREBUILT_IMAGE_PATH" ]]; then
      mv "$DECOMPRESSED" "$PREBUILT_IMAGE_PATH"
    fi
  else
    echo "Prebuilt image already at $PREBUILT_IMAGE_PATH"
  fi

  echo "Copying prebuilt image to $IMAGE_PATH"
  rm -f "$IMAGE_PATH"
  cp --reflink=auto "$PREBUILT_IMAGE_PATH" "$IMAGE_PATH"
else
  if [[ ! -f "$ISO_DOWNLOAD_PATH" ]]; then
    echo "Downloading ISO from $ISO_URL..."
    curl -L --retry 3 --retry-delay 5 --output "$ISO_DOWNLOAD_PATH" "$ISO_URL"
  else
    echo "ISO download already exists at $ISO_DOWNLOAD_PATH"
  fi

  if [[ -n "$ISO_SHA" ]]; then
    printf "%s  %s\n" "$ISO_SHA" "$ISO_DOWNLOAD_PATH" | sha256sum -c -
  fi

  if [[ "$NEEDS_DECOMPRESS" == true ]]; then
    if [[ ! -f "$ISO_PATH" ]]; then
      echo "Decompressing ISO to $ISO_PATH..."
      gzip --decompress --keep --force "$ISO_DOWNLOAD_PATH"
    else
      echo "Decompressed ISO already at $ISO_PATH"
    fi
  else
    ISO_PATH="$ISO_DOWNLOAD_PATH"
  fi

  if [[ -f "$IMAGE_PATH" ]]; then
    echo "Disk image already exists at $IMAGE_PATH (remove it to recreate)"
  else
    echo "Creating disk image $IMAGE_PATH (size $DISK_SIZE)..."
    qemu-img create -f qcow2 "$IMAGE_PATH" "$DISK_SIZE"
  fi
fi

cat <<EOF
Plan 9 setup complete.

Next steps:
 1. Boot the installer with 'tools/plan9-qemu/run.sh --install' (the script will use $ISO_PATH).
 2. Once installation finishes, shut down Plan 9 and rerun 'tools/plan9-qemu/run.sh' to boot the installed image.
 3. Share host directories via PLAN9_QEMU_SHARED_DIR or connect over the forwarded 9P port (default host 1564→guest 564).
EOF
