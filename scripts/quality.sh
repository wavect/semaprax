#!/usr/bin/env sh
set -eu

usage() {
    cat <<'EOF'
Usage: scripts/quality.sh [--plan] [quick|changed|full] [exact-changed-path ...]

Profiles:
  quick    Fast advisory checks for early local feedback
  changed  Route the complete Git change set to the narrowest safe gate set
  full     Run the complete repository gate (default)

Options:
  -n, --plan  Print and validate the routed gate plan without executing it
  -h, --help  Show this help

Path arguments are accepted only with the changed profile and must exactly
match the complete change set discovered from Git.
EOF
}

plan_only=0
case "${1:-}" in
    -h|--help) usage; exit 0 ;;
    -n|--plan) plan_only=1; shift ;;
esac

requested=${1:-full}
if [ "$#" -gt 0 ]; then shift; fi

case "${1:-}" in
    -h|--help) usage; exit 0 ;;
    -n|--plan) plan_only=1; shift ;;
esac

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
surface_rank=0
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
                changed:7:test-cli|changed:7:test-editor|changed:8:test-editor)
                    # Surface gates follow the base seven in one fixed order,
                    # each at most once: test-cli, then test-editor.
                    case "$first" in test-cli) rank=1 ;; test-editor) rank=2 ;; esac
                    [ "$rank" -gt "$surface_rank" ] || { echo "quality plan repeats or reorders surface gate: $first" >&2; exit 2; }
                    surface_rank=$rank
                    ;;
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

case "$effective:$gate_count" in quick:4|changed:7|changed:8|changed:9|full:12) ;; *) gate_count=0 ;; esac
[ "$schema_seen" -eq 1 ] && [ "$requested_seen" -eq 1 ] && [ "$effective_seen" -eq 1 ] && [ "$reason_seen" -eq 1 ] && [ "$base_seen" -eq 1 ] && [ "$gate_count" -gt 0 ] && [ "$ended" -eq 1 ] || {
    echo "quality plan is incomplete" >&2
    exit 2
}

if [ "$plan_only" -eq 1 ]; then
    exit 0
fi

# Dispatch only the exact, validated gate identifiers emitted by the plan.
while IFS="$tab" read -r kind gate _rest; do
    [ "$kind" = gate ] || continue
    printf '==> %s\n' "$gate" >&2
    case "$gate" in
        diff-check) git diff --check ;;
        fmt-check) cargo fmt --all --check ;;
        check-workspace) cargo check --locked --workspace --all-targets --all-features ;;
        test-advisory) cargo test --locked -p semaprax --all-features --test documentation --test examples --test agent_economics --test quality_routing ;;
        clippy-package) cargo clippy --locked -p semaprax --all-targets --all-features -- -D warnings ;;
        test-agent-context) cargo test --locked -p semaprax --all-features --test compiler --test agent_context --test agent_economics --test quality_routing --test documentation --test examples ;;
        rustdoc-package) RUSTDOCFLAGS="-D warnings" cargo doc --locked -p semaprax --all-features --no-deps ;;
        test-cli)
            cargo test --locked -p semaprax --all-features --test cli_help_surface_v1 --test cli_check_routing_v1 --test quickstart_v1 --test project_cli_v1 --test projections --test documentation
            cargo test --locked -p semaprax-toolchain --test cli_help_surface_v1
            ;;
        test-editor)
            (cd editors/vscode && node --test test/*.test.js)
            cargo test --locked -p semaprax --all-features --test documentation
            ;;
        clippy-workspace) cargo clippy --locked --workspace --all-targets --all-features -- -D warnings ;;
        test-workspace) cargo test --locked --workspace --all-targets --all-features ;;
        doctest-workspace) cargo test --locked --workspace --all-features --doc ;;
        rustdoc-workspace) RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps ;;
        build-release) cargo build --locked --workspace --release ;;
        package) cargo package --locked --allow-dirty -p semaprax ;;
        example-checks)
            for source in meaning ownership lifecycle control_flow records native_callable chars integers_i32 bytes_u8 explicit_mutation string_ops field_mutation while_loops inheritance; do
                cargo run --locked -p semaprax -- check "examples/$source.spx"
            done
            cargo run --locked -p semaprax -- check examples/calculator-rust/callback.spx
            ;;
        example-fmt)
            for source in meaning effects ownership lifecycle control_flow records native_callable chars integers_i32 bytes_u8 explicit_mutation string_ops field_mutation while_loops inheritance; do
                cargo run --locked -p semaprax -- fmt "examples/$source.spx" --check
            done
            cargo run --locked -p semaprax -- fmt examples/calculator-rust/callback.spx --check
            ;;
        *) echo "validated quality gate changed during dispatch: $gate" >&2; exit 2 ;;
    esac
done <"$plan_file"
