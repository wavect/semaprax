#!/bin/sh
# Source-only RI-15 blocker reproduction.  This script never invokes Cargo or
# rustup and never downloads a toolchain.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
rustc=${RUSTC:-rustc}
expected_commit=88d9e12ae178fab0fb5cc050a94da85685d449ea
expected_host=aarch64-apple-darwin

version=$("$rustc" -Vv)
printf '%s\n' "$version"
printf '%s\n' "$version" | grep -F "commit-hash: $expected_commit" >/dev/null
printf '%s\n' "$version" | grep -F "host: $expected_host" >/dev/null

scratch=$(mktemp -d "${TMPDIR:-/tmp}/semaprax-ri15-rustc-private.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM

set +e
RUSTC_BOOTSTRAP=1 "$rustc" --edition=2021 "$root/src/main.rs" \
  --out-dir "$scratch" >"$scratch/stderr" 2>&1
status=$?
set -e
cat "$scratch/stderr"

if [ "$status" -eq 0 ]; then
  echo "RI-15 blocker unexpectedly compiled; this does not prove interop" >&2
  exit 1
fi
grep -F "can't find crate for" "$scratch/stderr" | grep -F rustc_interface >/dev/null || {
  echo "RI-15 blocker did not report the expected missing rustc_interface crate" >&2
  exit 1
}
echo "RI-15 blocker reproduced: rustc_interface is unavailable on this pinned distribution" >&2
