#!/bin/sh
# Execute LAW-14's physical Lean/Z3 selector.  This is intentionally strict:
# an absent pin is a failed setup, never an ignored test reported as green.
set -eu

law14_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
for law14_var in \
    SEMAPRAX_LAW_LEAN \
    SEMAPRAX_LAW_LEAN_VERSION \
    SEMAPRAX_LAW_Z3 \
    SEMAPRAX_LAW_Z3_VERSION
do
    eval "law14_value=\${$law14_var-}"
    if [ -z "$law14_value" ]; then
        echo "LAW-14 requires $law14_var" >&2
        exit 2
    fi
done

for law14_path in "$SEMAPRAX_LAW_LEAN" "$SEMAPRAX_LAW_Z3"; do
    case "$law14_path" in
        /*) ;;
        *) echo "LAW-14 tool path must be absolute: $law14_path" >&2; exit 2 ;;
    esac
    if [ ! -x "$law14_path" ]; then
        echo "LAW-14 tool is unavailable: $law14_path" >&2
        exit 2
    fi
done

cd "$law14_root"
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$law14_root/target/ri05-owner}"
export RUSTFLAGS="${RUSTFLAGS:--C debuginfo=0}"
cargo test --offline --locked -j 1 -p semaprax --test workspace \
    project_assurance_manifest::law_set::law14_adversarial::law14_fast_mutation_corpus_rejects_named_weakening_before_authority -- --exact --test-threads=1
cargo test --offline --locked -j 1 -p semaprax --lib \
    project::candidate::candidate_assurance::tests::forged_nonempty_proof_reference_never_grants_formal_acceptance -- --exact --test-threads=1
cargo test --offline --locked -j 1 -p semaprax --lib \
    project::candidate::strict_law_assurance::law14_final_boundary_tests::law14_strict_publication_final_boundary_source_race_refuses_before_active -- --exact --test-threads=1
exec cargo test --offline --locked -j 1 -p semaprax --test workspace \
    project_assurance_manifest::law_set::installed_law::installed_native_law::installed_native_law_law14_adversarial_gate -- --ignored --exact --test-threads=1
