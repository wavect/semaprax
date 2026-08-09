#!/usr/bin/env sh
set -eu

# The Rust planner owns NUL-safe Git reconciliation and canonical path checks.
exec cargo run --locked --quiet -p semaprax -- quality-plan "$@"
