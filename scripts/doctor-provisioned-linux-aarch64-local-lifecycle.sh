#!/usr/bin/env bash
# Run the offline doctor's ignored physical lifecycle tests on native Linux
# AArch64, directly, without the x86-64-only production release/capsule
# pipeline.
#
# THIS IS NOT scripts/doctor-provisioned-linux-gate.py AND IS NOT THE
# REQUIRED GATE ISSUE #61 OR docs/DOCTOR-PROVISIONED-LINUX-GATE-V1.md
# DESCRIBE. That gate admits exactly one target, x86-64
# (`ADMITTED_MACHINE = "x86_64"` in doctor-provisioned-linux-gate.py, and its
# own --self-test asserts that "aarch64" is one of the architectures it must
# reject). This script exists precisely because that gate must never be
# widened to accept AArch64: issue #61's own acceptance criteria require
# AArch64 support to stay separately tracked and never generalized from one
# Linux environment. Nothing this script produces changes WP-05's status in
# docs/COMPLETION-MATRIX.md, changes docs/DOCTOR-PROVISIONED-LINUX-GATE-V1.md,
# or may be cited as x86-64 evidence.
#
# What it covers: all twenty-six `#[ignore]`d lifecycle fixtures of the two
# owning Rust suites, thirteen each, including the two real-distribution
# fixtures (`provisioned_real_clang_node_rust_distributions` and
# `real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions`)
# against real AArch64 Clang, Node and Rust carriers. Every test name is
# supplied with `--exact`; adding a matching ignored test cannot silently
# widen this probe. The fixtures are built and run as native AArch64 binaries,
# inside a real (non-emulated) AArch64 Linux kernel, with the fixed namespace
# acknowledgements the fixtures themselves assert on. It reuses the existing
# hostile fixtures unmodified; it adds no fixture and weakens none.
#
# The real carriers come from scripts/doctor-provisioned-linux-aarch64-carriers.sh,
# the only network-using step. Its carriers.env is this script's one argument:
# SEMAPRAX_DOCTOR_REAL_BUNDLE, SEMAPRAX_DOCTOR_REAL_SELECTOR and the three
# SEMAPRAX_DOCTOR_EXPECTED_*_DETAIL values. This script invents none of them;
# without them it refuses before building anything.
#
# What it does NOT cover: a signed AArch64 release package. The twenty-six
# fixtures consume the current-head worker, launcher and collector directly,
# so no release capsule is involved and none is claimed.
#
# The kernel must provide /proc/<pid>/task/<tid>/children
# (CONFIG_PROC_CHILDREN); the supervisor-death fixture observes the tool
# through it. Apple Container's default kernel lacks it, so issue #334 ran on
# a rebuild of that kernel with only this option changed.
#
# Usage from macOS on Apple Silicon (the Linux VM is native AArch64, not
# emulated), with Docker Desktop:
#
#   docker run --rm --privileged --cgroupns=private \
#     -v "$(pwd)":/repo -v <cargo-registry-cache>:/usr/local/cargo/registry \
#     rust:1-slim-bookworm bash -c '
#       bash /repo/scripts/doctor-provisioned-linux-aarch64-carriers.sh /carriers &&
#       bash /repo/scripts/doctor-provisioned-linux-aarch64-local-lifecycle.sh /carriers/carriers.env'
#
# or with Apple Container: `container run --cap-add ALL -k <kernel-with-PROC_CHILDREN> ...`.
# Directly on a disposable AArch64 Linux host, run both scripts as-is from the
# repository root.
#
# Refuses (does not skip) unless Linux on AArch64, cargo is on PATH,
# unshare(1) can create a private user+mount namespace, and every real-carrier
# input is supplied -- missing provisioning is a failure here too, exactly as
# issue #61 requires of the x86-64 gate.

set -o errexit
set -o nounset
set -o pipefail

fail() {
	printf 'error: %s\n' "$1" >&2
	printf 'This is a failure, not a skip.\n' >&2
	exit 1
}

require_host() {
	local system machine
	system="$(uname -s)"
	machine="$(uname -m)"
	[ "${system}" = "Linux" ] || fail "host system is '${system}', not 'Linux'"
	case "${machine}" in
	aarch64 | arm64) ;;
	*) fail "host architecture is '${machine}', not AArch64; use doctor-provisioned-linux-provision.sh for x86-64" ;;
	esac
}

require_user_namespaces() {
	command -v unshare >/dev/null 2>&1 || fail "unshare(1) is not installed"
	unshare --user --map-root-user --mount --net --ipc --uts true >/dev/null 2>&1 ||
		fail "cannot create a private user+mount namespace as this user"
}

require_cargo() {
	command -v cargo >/dev/null 2>&1 || fail "cargo is not on PATH"
}

readonly REPOSITORY="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly TARGET_DIR="${CARGO_TARGET_DIR:-${REPOSITORY}/target-aarch64-local}"

# This is deliberately an enumerated plan, rather than a broad libtest filter:
# the result is meaningful only for this exact set of twenty-six fixtures.
readonly -a PLATFORM_LIFECYCLE_TESTS=(
	"doctor::offline_root::linux::tests::provisioned_close_uncertainty_is_fail_stop"
	"doctor::offline_root::linux::tests::provisioned_detached_root_bytes_modes_and_read_only"
	"doctor::offline_root::linux::tests::provisioned_metadata_mismatches_feed_actual_admission"
	"doctor::offline_root::linux::tests::provisioned_setup_and_exact_write_failures_return_no_root"
	"doctor::offline_root::linux::tests::provisioned_wrong_page_cost_stops_before_tree_writes"
	"doctor::offline_worker::tests::hostile::provisioned_capability_operations_and_process_creation_are_denied"
	"doctor::offline_worker::tests::hostile::provisioned_root_hides_real_outside_file_and_rejects_write_opens"
	"doctor::offline_worker::tests::hostile::provisioned_stdin_is_eof_and_nonstandard_descriptors_are_closed"
	"doctor::offline_worker::tests::lifecycle::post_exec_capabilities_and_supervisor_death_are_observed_externally"
	"doctor::offline_worker::tests::provisioned_materializer_exec_and_socket_denial"
	"doctor::offline_worker::tests::provisioned_missing_role_bad_hash_and_invalid_request_emit_no_frame"
	"doctor::offline_worker::tests::provisioned_overflow_and_timeout_publish_only_settled_failure"
	"doctor::offline_worker::tests::provisioned_real_clang_node_rust_distributions"
)
readonly -a COLLECTOR_LIFECYCLE_TESTS=(
	"actual_worker_materializes_executes_and_settles_before_canonical_report"
	"complete_frame_and_capture_eof_each_still_require_worker_exit"
	"complete_literal_frame_followed_by_nonzero_exit_never_becomes_a_report"
	"created_handoff::production_created_native_and_all_files_reach_worker_and_reject_digest_drift"
	"launched_handoff::production_launcher_rejects_both_image_defects_and_digest_drift"
	"launched_handoff::production_launcher_rejects_structural_collector_with_missing_loader"
	"launched_handoff::production_launcher_reports_native_and_all_from_literal_transport_files"
	"literal_reply_surrogates_reject_cross_binding_and_malformed_frames"
	"nonchild::nonchild_pidfd_rejects_without_killing_or_stopping_the_owned_sentinel"
	"physical_reports::all_three_roles_settle_and_tool_failure_is_an_ordinary_exit_one_report"
	"physical_reports::closed_report_sink_fails_after_collection_without_successful_delivery"
	"prepared_handoff::prepared_native_and_all_role_handoffs_preserve_literal_wire_and_reject_transport_drift"
	"real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions"
)
readonly TRACKING_FIXTURE_COUNT=26
readonly -a REAL_CARRIER_KEYS=(
	"SEMAPRAX_DOCTOR_REAL_BUNDLE"
	"SEMAPRAX_DOCTOR_REAL_SELECTOR"
	"SEMAPRAX_DOCTOR_EXPECTED_CLANG_DETAIL"
	"SEMAPRAX_DOCTOR_EXPECTED_NODE_DETAIL"
	"SEMAPRAX_DOCTOR_EXPECTED_RUST_DETAIL"
)

require_tracking_plan() {
	[ "${#PLATFORM_LIFECYCLE_TESTS[@]}" -eq 13 ] || fail "platform lifecycle plan is not 13 fixtures"
	[ "${#COLLECTOR_LIFECYCLE_TESTS[@]}" -eq 13 ] || fail "collector lifecycle plan is not 13 fixtures"
	[ "$(( ${#PLATFORM_LIFECYCLE_TESTS[@]} + ${#COLLECTOR_LIFECYCLE_TESTS[@]} ))" -eq "${TRACKING_FIXTURE_COUNT}" ] || \
		fail "AArch64 tracking plan is not ${TRACKING_FIXTURE_COUNT} fixtures"
}

# carriers.env holds KEY=VALUE lines whose values may contain spaces and
# parentheses, so it is parsed rather than sourced: exactly the five
# real-carrier keys, each once, each nonempty, and nothing else.
load_real_carriers() {
	local file="$1" line key value seen=""
	[ -f "${file}" ] || fail "real-carrier file ${file} does not exist"
	while IFS= read -r line || [ -n "${line}" ]; do
		key="${line%%=*}"
		value="${line#*=}"
		[ "${key}" != "${line}" ] && [ -n "${value}" ] || fail "malformed real-carrier line: ${line}"
		case " ${REAL_CARRIER_KEYS[*]} " in
		*" ${key} "*) ;;
		*) fail "unexpected real-carrier key ${key}" ;;
		esac
		case " ${seen} " in
		*" ${key} "*) fail "duplicate real-carrier key ${key}" ;;
		esac
		seen="${seen} ${key}"
		export "${key}=${value}"
	done <"${file}"
	for key in "${REAL_CARRIER_KEYS[@]}"; do
		[ -n "${!key:-}" ] || fail "real-carrier input ${key} is missing; provision it with doctor-provisioned-linux-aarch64-carriers.sh"
	done
	[ -f "${SEMAPRAX_DOCTOR_REAL_BUNDLE}" ] || fail "real bundle ${SEMAPRAX_DOCTOR_REAL_BUNDLE} does not exist"
}

main() {
	require_host
	require_user_namespaces
	require_cargo
	require_tracking_plan
	[ "$#" -eq 1 ] || fail "usage: $0 <carriers.env from doctor-provisioned-linux-aarch64-carriers.sh>"
	load_real_carriers "$1"
	cd "${REPOSITORY}"
	export CARGO_TARGET_DIR="${TARGET_DIR}"

	echo "== building current-head worker, launcher and collector for $(uname -m) =="
	cargo build --locked -p semaprax-native-rust-interop-platform-sys \
		--bin semaprax-doctor-worker --bin semaprax-doctor-launcher
	cargo build --locked -p semaprax-doctor-collector --bin semaprax-doctor-collector

	export SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT="private-mapped-user-mount-clean-worker-cgroup-v1"
	export SEMAPRAX_DOCTOR_ROOT_TEST_CONTEXT="private-user-mount-v1"
	export SEMAPRAX_DOCTOR_WORKER="${TARGET_DIR}/debug/semaprax-doctor-worker"
	export SEMAPRAX_DOCTOR_LAUNCHER="${TARGET_DIR}/debug/semaprax-doctor-launcher"
	export SEMAPRAX_DOCTOR_COLLECTOR="${TARGET_DIR}/debug/semaprax-doctor-collector"

	echo "== running the platform-sys-lib ignored lifecycle suite, real carriers included =="
	unshare --user --map-root-user --mount --net --ipc --uts -- \
		cargo test --locked --offline -p semaprax-native-rust-interop-platform-sys --lib -- \
		--ignored --exact --test-threads=1 \
		"${PLATFORM_LIFECYCLE_TESTS[@]}"
	echo "== running the doctor-collector 'provisioned' ignored lifecycle suite, real carriers included =="
	unshare --user --map-root-user --mount --net --ipc --uts -- \
		cargo test --locked --offline -p semaprax-doctor-collector --test provisioned -- \
		--ignored --exact --test-threads=1 \
		"${COLLECTOR_LIFECYCLE_TESTS[@]}"

	echo "== done: ${TRACKING_FIXTURE_COUNT} AArch64 lifecycle fixtures passed with real carriers =="
	echo "   It is not the x86-64 gate or a signed-release gate, does not change"
	echo "   WP-05, and must never be cited as x86-64 confinement evidence."
}

main "$@"
