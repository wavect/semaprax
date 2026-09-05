#!/usr/bin/env bash
# Provision the isolated Linux environment the offline doctor lifecycle gate
# requires, then run the gate inside it.
#
# STATUS: authored and unrun on Linux. This wrapper has never executed on a
# Linux x86-64 host. It was written and self-tested on macOS/arm64, where every
# path below refuses. Do not describe it as verified confinement until a
# maintainer has run it on a real disposable host and recorded the evidence.
#
# The gate itself (scripts/doctor-provisioned-linux-gate.py) independently
# re-checks every property this wrapper claims to establish. Nothing here is
# trusted: the acknowledgement variables are provisioner assertions, and the
# gate still observes the kernel, the delegated scope and the images for itself.
# If this wrapper is wrong, the gate fails closed rather than passing.
#
# Usage:
#   scripts/doctor-provisioned-linux-provision.sh --release <dir> --evidence <path>
#
#   --release   Unpacked doctor release directory, OUTSIDE the checkout, holding
#               the launcher, worker and collector images plus the real bundle.
#   --evidence  Where the gate writes its evidence JSON.
#   --keep      Do not delete the delegated cgroup scope on exit (debugging).
#
# Refuses unless every one of these holds, because a skip is never a passing
# confinement result:
#   * Linux on x86-64
#   * cgroup-v2 unified hierarchy mounted
#   * cpu, memory and pids controllers delegable to this user
#   * unprivileged user namespaces permitted
#   * the release directory lies outside this checkout and carries all images

set -o errexit
set -o nounset
set -o pipefail

readonly REPOSITORY="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly GATE="${REPOSITORY}/scripts/doctor-provisioned-linux-gate.py"

# Matches REQUIRED_CONTEXT_ENVIRONMENT in the gate.
readonly WORKER_CONTEXT="private-mapped-user-mount-clean-worker-cgroup-v1"
readonly ROOT_CONTEXT="private-user-mount-v1"

# Matches REQUIRED_CGROUP_CONTROLLERS in the gate.
readonly CONTROLLERS=(cpu memory pids)

RELEASE=""
EVIDENCE=""
KEEP="no"
SCOPE=""

fail() {
	printf 'error: %s\n' "$1" >&2
	printf 'This is a failure, not a skip. Missing provisioning is never a passing confinement result.\n' >&2
	exit 1
}

usage() {
	sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
	exit 2
}

parse_arguments() {
	while [ "$#" -gt 0 ]; do
		case "$1" in
		--release)
			[ "$#" -ge 2 ] || fail "--release needs a directory"
			RELEASE="$2"
			shift 2
			;;
		--evidence)
			[ "$#" -ge 2 ] || fail "--evidence needs a path"
			EVIDENCE="$2"
			shift 2
			;;
		--keep)
			KEEP="yes"
			shift
			;;
		--help | -h)
			usage
			;;
		*)
			fail "unknown argument: $1"
			;;
		esac
	done
	[ -n "${RELEASE}" ] || fail "--release is required"
	[ -n "${EVIDENCE}" ] || fail "--evidence is required"
}

require_host() {
	local system machine
	system="$(uname -s)"
	machine="$(uname -m)"
	[ "${system}" = "Linux" ] ||
		fail "host system is '${system}', not 'Linux'; the doctor lifecycle gate is Linux-only"
	case "${machine}" in
	x86_64 | amd64) ;;
	*) fail "host architecture is '${machine}', not x86-64; the sealed images are x86-64 ELF" ;;
	esac
}

require_user_namespaces() {
	local knob="/proc/sys/kernel/unprivileged_userns_clone"
	if [ -r "${knob}" ] && [ "$(cat "${knob}")" != "1" ]; then
		fail "unprivileged user namespaces are disabled (${knob} is 0)"
	fi
	command -v unshare >/dev/null 2>&1 ||
		fail "unshare(1) is not installed; it creates the private user and mount namespaces"
	unshare --user --map-root-user true >/dev/null 2>&1 ||
		fail "cannot create a private user namespace as this user"
}

require_cgroup_v2() {
	local unified="/sys/fs/cgroup"
	[ -e "${unified}/cgroup.controllers" ] ||
		fail "cgroup-v2 unified hierarchy is not mounted at ${unified}"
	local available
	available="$(cat "${unified}/cgroup.controllers")"
	local controller
	for controller in "${CONTROLLERS[@]}"; do
		case " ${available} " in
		*" ${controller} "*) ;;
		*) fail "cgroup controller '${controller}' is not available at ${unified}" ;;
		esac
	done
}

# Create an empty delegated scope the provisioner can install limits into and
# whose emptiness proves settlement. The gate rereads every one of these files.
create_scope() {
	local parent="/sys/fs/cgroup/${SEMAPRAX_DOCTOR_GATE_PARENT:-user.slice}"
	[ -d "${parent}" ] || fail "delegated cgroup parent '${parent}' does not exist"
	[ -w "${parent}/cgroup.subtree_control" ] ||
		fail "cannot write '${parent}/cgroup.subtree_control'; the scope is not delegated to this user"
	local controller
	for controller in "${CONTROLLERS[@]}"; do
		printf '+%s\n' "${controller}" >"${parent}/cgroup.subtree_control" 2>/dev/null ||
			fail "cannot delegate controller '${controller}' under '${parent}'"
	done
	SCOPE="${parent}/semaprax-doctor-gate.$$"
	mkdir "${SCOPE}" || fail "cannot create the delegated scope '${SCOPE}'"
	[ -e "${SCOPE}/cgroup.kill" ] ||
		fail "delegated scope '${SCOPE}' exposes no cgroup.kill; kernel is too old for this contract"
}

remove_scope() {
	[ -n "${SCOPE}" ] || return 0
	[ "${KEEP}" = "yes" ] && return 0
	[ -d "${SCOPE}" ] || return 0
	printf '1\n' >"${SCOPE}/cgroup.kill" 2>/dev/null || true
	rmdir "${SCOPE}" 2>/dev/null || true
}

require_release() {
	[ -d "${RELEASE}" ] || fail "release directory '${RELEASE}' does not exist"
	local resolved
	resolved="$(cd -- "${RELEASE}" && pwd -P)"
	case "${resolved}/" in
	"${REPOSITORY}"/*)
		fail "release '${resolved}' is inside the checkout; unpack it outside so the gate cannot read build outputs as source"
		;;
	esac
	RELEASE="${resolved}"
	local image
	for image in launcher worker collector; do
		[ -x "${RELEASE}/${image}" ] ||
			fail "release is missing an executable '${image}' image at ${RELEASE}/${image}"
	done
	[ -e "${RELEASE}/bundle" ] ||
		fail "release is missing the real distribution bundle at ${RELEASE}/bundle"
}

main() {
	parse_arguments "$@"
	require_host
	require_user_namespaces
	require_cgroup_v2
	require_release
	trap remove_scope EXIT
	create_scope

	# Acknowledgements the fixtures read. The gate treats these as assertions,
	# never as proof, and independently observes what it can.
	export SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT="${WORKER_CONTEXT}"
	export SEMAPRAX_DOCTOR_ROOT_TEST_CONTEXT="${ROOT_CONTEXT}"
	export SEMAPRAX_DOCTOR_LAUNCHER="${RELEASE}/launcher"
	export SEMAPRAX_DOCTOR_WORKER="${RELEASE}/worker"
	export SEMAPRAX_DOCTOR_COLLECTOR="${RELEASE}/collector"
	export SEMAPRAX_DOCTOR_REAL_BUNDLE="${RELEASE}/bundle"
	export SEMAPRAX_DOCTOR_GATE_CGROUP="${SCOPE}"

	# Every remaining variable is the operator's to supply, because only the
	# operator knows the disposable host and the fixture's expectations:
	#   SEMAPRAX_DOCTOR_REAL_SELECTOR
	#   SEMAPRAX_DOCTOR_EXPECTED_{CLANG,NODE,RUST}_DETAIL
	#   SEMAPRAX_DOCTOR_GATE_DISPOSABLE=yes
	# The gate refuses when any is absent, so this wrapper does not invent them.

	exec unshare --user --map-root-user --mount --net --ipc --uts -- \
		python3 "${GATE}" --evidence "${EVIDENCE}"
}

main "$@"
