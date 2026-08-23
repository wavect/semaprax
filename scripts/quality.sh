#!/usr/bin/env sh
set -eu

requested=${1:-full}
if [ "$#" -gt 0 ]; then
    shift
fi

plan_file=$(mktemp "${TMPDIR:-/tmp}/semaprax-quality-plan.XXXXXX")
trap 'rm -f "$plan_file"' EXIT HUP INT TERM
sh scripts/quality-route.sh "$requested" "$@" >"$plan_file"
cat "$plan_file"

tab=$(printf '\t')
schema_seen=0
requested_seen=0
effective_seen=0
reason_seen=0
base_seen=0
gate_count=0
ended=0
effective=
while IFS="$tab" read -r kind first second third extra; do
    if [ "$ended" -eq 1 ]; then
        echo "quality plan contains records after its end marker" >&2
        exit 2
    fi
    case "$kind" in
        schema)
            [ "$schema_seen" -eq 0 ] && [ "$first" = semaprax.quality-route.v2 ] && [ -z "${second}${third}${extra}" ] || exit 2
            schema_seen=1
            ;;
        requested)
            [ "$requested_seen" -eq 0 ] && [ "$first" = "$requested" ] && [ -z "${second}${third}${extra}" ] || exit 2
            requested_seen=1
            ;;
        effective)
            [ "$effective_seen" -eq 0 ] && [ -z "${second}${third}${extra}" ] || exit 2
            case "$requested:$first" in
                quick:quick|changed:changed|changed:full|full:full) effective=$first ;;
                *) exit 2 ;;
            esac
            effective_seen=1
            ;;
        reason)
            [ "$reason_seen" -eq 0 ] && [ -n "$first" ] && [ -z "${second}${third}${extra}" ] || exit 2
            reason_seen=1
            ;;
        base)
            [ "$base_seen" -eq 0 ] && [ -n "$first" ] && [ -z "${second}${third}${extra}" ] || exit 2
            case "$requested:$first" in
                quick:not-applicable|full:not-applicable) ;;
                changed:*)
                    case "$first" in
                        *[!0-9a-f]*|'') exit 2 ;;
                    esac
                    [ "${#first}" -eq 40 ] || [ "${#first}" -eq 64 ] || exit 2
                    ;;
                *) exit 2 ;;
            esac
            base_seen=1
            ;;
        path)
            [ -n "$first" ] && [ -n "$second" ] && [ -n "$third" ] && [ -z "$extra" ] || exit 2
            ;;
        gate)
            [ -z "${second}${third}${extra}" ] || exit 2
            case "$effective:$gate_count:$first" in
                quick:0:diff-check|quick:1:fmt-check|quick:2:check-workspace|quick:3:test-advisory) ;;
                changed:0:diff-check|changed:1:fmt-check|changed:2:check-workspace|changed:3:test-advisory|changed:4:clippy-package|changed:5:test-agent-context|changed:6:rustdoc-package) ;;
                full:0:diff-check|full:1:fmt-check|full:2:check-workspace|full:3:test-advisory|full:4:clippy-workspace|full:5:test-workspace|full:6:doctest-workspace|full:7:rustdoc-workspace|full:8:build-release|full:9:package|full:10:example-checks|full:11:example-fmt) ;;
                *) echo "quality plan gate sequence does not match effective profile: $first" >&2; exit 2 ;;
            esac
            gate_count=$((gate_count + 1))
            ;;
        end)
            [ "$first" = quality-plan ] && [ -z "${second}${third}${extra}" ] || exit 2
            ended=1
            ;;
        *) echo "quality plan contains unknown record: $kind" >&2; exit 2 ;;
    esac
done <"$plan_file"

case "$effective:$gate_count" in quick:4|changed:7|full:12) ;; *) gate_count=0 ;; esac
[ "$schema_seen" -eq 1 ] && [ "$requested_seen" -eq 1 ] && [ "$effective_seen" -eq 1 ] && [ "$reason_seen" -eq 1 ] && [ "$base_seen" -eq 1 ] && [ "$gate_count" -gt 0 ] && [ "$ended" -eq 1 ] || {
    echo "quality plan is incomplete" >&2
    exit 2
}

# Dispatch only the exact, validated gate identifiers emitted by the plan.
while IFS="$tab" read -r kind gate _rest; do
    [ "$kind" = gate ] || continue
    case "$gate" in
        diff-check) git diff --check ;;
        fmt-check) cargo fmt --all --check ;;
        check-workspace) cargo check --locked --workspace --all-targets --all-features ;;
        test-advisory) cargo test --locked -p semaprax --all-features --test documentation --test examples --test agent_economics --test quality_routing ;;
        clippy-package) cargo clippy --locked -p semaprax --all-targets --all-features -- -D warnings ;;
        test-agent-context) cargo test --locked -p semaprax --all-features --test compiler --test agent_context --test agent_economics --test quality_routing --test documentation --test examples ;;
        rustdoc-package) RUSTDOCFLAGS="-D warnings" cargo doc --locked -p semaprax --all-features --no-deps ;;
        clippy-workspace) cargo clippy --locked --workspace --all-targets --all-features -- -D warnings ;;
        test-workspace) cargo test --locked --workspace --all-targets --all-features ;;
        doctest-workspace) cargo test --locked --workspace --all-features --doc ;;
        rustdoc-workspace) RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps ;;
        build-release) cargo build --locked --workspace --release ;;
        package) cargo package --locked --allow-dirty -p semaprax ;;
        example-checks)
            for source in meaning ownership lifecycle control_flow records native_callable chars integers_i32 bytes_u8 explicit_mutation; do
                cargo run --locked -p semaprax -- check "examples/$source.spx"
            done
            ;;
        example-fmt)
            for source in meaning effects ownership lifecycle control_flow records native_callable chars integers_i32 bytes_u8 explicit_mutation; do
                cargo run --locked -p semaprax -- fmt "examples/$source.spx" --check
            done
            ;;
        *) echo "validated quality gate changed during dispatch: $gate" >&2; exit 2 ;;
    esac
done <"$plan_file"
