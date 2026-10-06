#!/bin/sh
# MR-13 real routing matrix (opt-in). Prints the cost ceiling first and refuses
# unless every prerequisite is present: the operator's real cell executor
# (SEMAPRAX_MATRIX_EXECUTOR, speaking semaprax.harness-routing-cell.v1), at
# least two available generation profiles, and SEMAPRAX_MATRIX_MAX_USD at or
# above the printed ceiling. Learned adapters run only when their own
# requirements (credentials, worker endpoints, hardware tags in
# SEMAPRAX_MATRIX_HARDWARE) are met; the others stay `unavailable`.
# Never run in CI. Output: $SEMAPRAX_MATRIX_OUT (default ./routing-matrix-real).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
harness() { cargo run --locked --quiet -p semaprax-harness -- "$@"; }
common="--registry $here/registry.json --tasks $here/tasks.json --gate-spec $here/gate-spec.json --repo $repo"
cd "$repo"
# shellcheck disable=SC2086
harness bench routing-matrix $common --plan
if [ -z "${SEMAPRAX_MATRIX_EXECUTOR:-}" ]; then
  echo "refused: SEMAPRAX_MATRIX_EXECUTOR is not set (a fixture never counts as a real run)" >&2
  exit 2
fi
if [ -z "${SEMAPRAX_MATRIX_MAX_USD:-}" ]; then
  echo "refused: set SEMAPRAX_MATRIX_MAX_USD to at least the ceiling printed above" >&2
  exit 2
fi
# shellcheck disable=SC2086
harness bench routing-matrix $common --real --max-usd "$SEMAPRAX_MATRIX_MAX_USD" \
  --hardware "${SEMAPRAX_MATRIX_HARDWARE_TEXT:-unspecified}" \
  --pin "commit=$(git rev-parse HEAD)" \
  --out "${SEMAPRAX_MATRIX_OUT:-$repo/routing-matrix-real}"
