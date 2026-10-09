#!/bin/sh
set -eu
cd "$(dirname "$0")"
compiler=${SEMAPRAX_BIN:-/Users/kevin/.codex/benchmark-binaries/semaprax-398b051e6}
"$compiler" check semaprax.toml
rm -f loglens
"$compiler" build --manifest-path semaprax.toml --target native --output loglens
