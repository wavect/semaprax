#!/bin/sh
set -eu
printf '%s\n' "$0" >> "$FIXTURE_ROOT/smoke-calls"
case "$1" in
    --version) printf 'semaprax %s (%s)\n' "$FIXTURE_VERSION" "$FIXTURE_COMMIT" ;;
    version) printf '{"schema":"semaprax.version.v1","version":"%s","commit":"%s","maturity":"pre-alpha","rust_min":"1.88"}\n' "$FIXTURE_VERSION" "$FIXTURE_COMMIT" ;;
    check) [ -f "$2" ] ;;
    run) [ -f "$2" ]; printf '42\n' ;;
    *) exit 19 ;;
esac
[ "$FAKE_SMOKE_FAIL" != "$1" ] || exit 23
