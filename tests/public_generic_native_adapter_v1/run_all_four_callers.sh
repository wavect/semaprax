#!/bin/sh
# Issue #172: one executable entry point that runs all four generated
# public-generic *calling* consumers and prints one aggregate result, so
# "all four callers execute" is a thing a reviewer can run rather than a
# claim spread across four separate harnesses that each half-support it.
#
# Runs, as independent `cargo test` invocations against the currently
# checked-out tree:
#   - Rust             (#156) rust_calling_consumer::generated_rust_calling_consumer_executes_against_the_real_native_provider
#   - C11              (#158) c_calling_consumer::generated_c_calling_consumer_executes_against_the_real_native_provider
#   - C++17            (#159) cxx_calling_consumer::generated_cxx_calling_consumer_executes_against_the_real_native_provider
#   - TypeScript/Wasm  (#157) typescript_calling_consumer::generated_typescript_calling_consumer_executes_against_a_real_wasm_module
#
# HONESTY NOTE, restated from docs/PUBLIC-GENERIC-CONSUMERS-V1.md and issue
# #229: the first three execute the generated consumer against a genuinely
# COMPILED native provider artifact (issue #154's `provider_body.c`, built by
# a real C compiler). The fourth executes against
# `tests/public_generic_wasm_adapter_v1/reference_wasm_module.rs`, a
# hand-assembled, clearly-labelled TEST-ONLY STAND-IN for a compiled Wasm
# provider -- no `.wasm` artifact implementing the Core Wasm provider ABI
# (open/input_prepare/call/result_export/release) exists anywhere in this
# repository yet (#229). This script never averages that gap away: its own
# PASS line for the fourth caller says so explicitly, and so does the
# summary at the end.
#
# Toolchain absence is reported as an explicit SKIP, never folded into a
# false PASS or FAIL, matching every harness's own existing convention
# (`fixture.rs`, `typescript_calling_consumer.rs`): `clang`/`clang++`/`cargo`
# are assumed present (this script does not skip on their absence, since
# every other native-adapter harness in this module already requires them
# unconditionally); `node` and a repository-pinned (5.8.3) `tsc` are
# probed and reported as a skip when missing.
#
# Usage: sh tests/public_generic_native_adapter_v1/run_all_four_callers.sh
# Exit code: 0 only if the three native callers pass AND (when node/tsc are
# available) the TypeScript/Wasm caller also passes against its stand-in
# module. A missing node/tsc toolchain does not affect the exit code, same
# as the underlying test's own skip behavior.

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"

native_bin=public_generic_native_adapter_v1
wasm_bin=public_generic_wasm_adapter_v1

rust_test="rust_calling_consumer::generated_rust_calling_consumer_executes_against_the_real_native_provider"
c_test="c_calling_consumer::generated_c_calling_consumer_executes_against_the_real_native_provider"
cxx_test="cxx_calling_consumer::generated_cxx_calling_consumer_executes_against_the_real_native_provider"
typescript_test="typescript_calling_consumer::generated_typescript_calling_consumer_executes_against_a_real_wasm_module"

overall_status=0

echo "== Native callers: Rust (#156), C11 (#158), C++17 (#159) against the real compiled native provider =="
if cargo test --locked --test "$native_bin" -- "$rust_test" "$c_test" "$cxx_test"; then
    echo "AGGREGATE $rust_test PASS (real native provider)"
    echo "AGGREGATE $c_test PASS (real native provider)"
    echo "AGGREGATE $cxx_test PASS (real native provider)"
else
    echo "AGGREGATE native-callers FAIL (see cargo test output above for which of the three failed)"
    overall_status=1
fi

echo
echo "== TypeScript/Wasm caller: (#157) =="
node_ok=0
if command -v node >/dev/null 2>&1 && node --version >/dev/null 2>&1; then
    node_ok=1
fi
tsc_bin=""
for candidate in "${SPX_PG_TSC:-}" tsc "$HOME/Library/pnpm/tsc" "$HOME/.local/share/pnpm/tsc"; do
    [ -n "$candidate" ] || continue
    if "$candidate" --version 2>/dev/null | grep -q '5\.8\.3'; then
        tsc_bin=$candidate
        break
    fi
done

if [ "$node_ok" = 1 ] && [ -n "$tsc_bin" ]; then
    if cargo test --locked --test "$wasm_bin" -- "$typescript_test"; then
        echo "AGGREGATE $typescript_test PASS (against a TEST-ONLY Wasm stand-in module, NOT a real compiled provider ABI -- see issue #229)"
    else
        echo "AGGREGATE $typescript_test FAIL"
        overall_status=1
    fi
else
    echo "AGGREGATE $typescript_test SKIPPED (node and/or repository-pinned tsc 5.8.3 not found)"
fi

echo
echo "== Summary =="
echo "3 of 4 generated calling consumers execute today against a genuinely compiled provider artifact: Rust (#156), C11 (#158), C++17 (#159)."
echo "The 4th, TypeScript/Wasm (#157), executes only against a hand-assembled, clearly-labelled test-only stand-in module (tests/public_generic_wasm_adapter_v1/reference_wasm_module.rs); issue #229 tracks shipping a real compiled Wasm provider artifact."
echo "\"All four callers execute against a real provider ABI\" is NOT an accurate claim today -- three do, one executes against a stand-in."

exit "$overall_status"
