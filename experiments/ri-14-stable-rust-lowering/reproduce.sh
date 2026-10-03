#!/bin/sh
# Direct stable-rustc fixture runner.  No Cargo, rustup, network, or downloads.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
rustc=${RUSTC:-rustc}
expected_commit=88d9e12ae178fab0fb5cc050a94da85685d449ea
expected_host=aarch64-apple-darwin

version=$("$rustc" -Vv)
printf '%s\n' "$version"
printf '%s\n' "$version" | grep -F "commit-hash: $expected_commit" >/dev/null
printf '%s\n' "$version" | grep -F "host: $expected_host" >/dev/null

scratch=$(mktemp -d "${TMPDIR:-/tmp}/semaprax-ri14-stable-rust.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
"$rustc" --edition=2021 -C panic=unwind "$root/src/main.rs" -o "$scratch/ri14"
"$scratch/ri14"
