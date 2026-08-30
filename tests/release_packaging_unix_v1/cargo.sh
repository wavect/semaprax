#!/bin/sh
set -eu
printf '%s\n' "$@" > "$FIXTURE_ROOT/cargo-arguments"
printf '%s' "$SEMAPRAX_BUILD_COMMIT" > "$FIXTURE_ROOT/cargo-commit"
[ "$FAKE_CARGO_FAIL" = 0 ] || exit 17
build_root=${CARGO_TARGET_DIR:-target}
while [ "$#" -gt 0 ]; do
    if [ "$1" = --target-dir ]; then
        shift
        build_root=$1
    fi
    shift
done
case "$build_root" in
    "$FIXTURE_ROOT/output with spaces/build-x86_64-unknown-linux-gnu"|"$FIXTURE_ROOT/ambient target ignored") ;;
    *) exit 18 ;;
esac
mkdir -p "$build_root/x86_64-unknown-linux-gnu/release"
cp "$FIXTURE_ROOT/cli" "$build_root/x86_64-unknown-linux-gnu/release/semaprax-full"
cp "$FIXTURE_ROOT/daemon" "$build_root/x86_64-unknown-linux-gnu/release/semapraxd"
