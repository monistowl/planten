#!/usr/bin/env bash
set -euo pipefail

echo "Checking for deprecated \`Command::cargo_bin\` uses..."
matches=$(rg --line-number "Command::cargo_bin" userspace/planten_coreutils/tests || true)
if [[ -n "$matches" ]]; then
  echo "Deprecated helper still used in tests:"
  echo "$matches"
  echo "Replace with \`cargo_bin!\` (see tests/bind.rs, tests/mount.rs, tests/persistence.rs)."
  exit 1
fi

echo "No deprecated \`Command::cargo_bin\` usages found."
