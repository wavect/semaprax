#!/usr/bin/env bash
# Per-requirement probe of a GitHub-HOSTED runner against the host
# preconditions of scripts/doctor-provisioned-linux-gate.py and its
# provisioning wrapper scripts/doctor-provisioned-linux-provision.sh.
#
# THIS IS A PROBE, NOT THE GATE. It runs none of the 26 ignored lifecycle
# fixtures and builds none of the sealed current-head images. A PASS here means
# only that the named host requirement is observably satisfiable on this
# runner. It is never a confinement result.
#
# No precondition is relaxed. Each check restates the requirement the gate or
# the wrapper already enforces and observes it directly.

set -o nounset
set -o pipefail

REPO="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
GATE="${REPO}/scripts/doctor-provisioned-linux-gate.py"
RESULTS=()
FAILED=0

record() { # record <PASS|FAIL> <requirement> <detail>
	RESULTS+=("$1|$2|$3")
	printf '[%s] %s -- %s\n' "$1" "$2" "$3"
	[ "$1" = "FAIL" ] && FAILED=1
	return 0
}

check() { # check <requirement> <detail-on-pass> <command...>
	local requirement="$1" detail="$2"
	shift 2
	if "$@" >/dev/null 2>&1; then
		record PASS "${requirement}" "${detail}"
	else
		record FAIL "${requirement}" "command failed: $*"
	fi
}

echo '### 1. Architecture and system'
[ "$(uname -s)" = "Linux" ] &&
	record PASS "host system is Linux" "$(uname -s)" ||
	record FAIL "host system is Linux" "$(uname -s)"
case "$(uname -m)" in
x86_64 | amd64) record PASS "host architecture is x86-64" "$(uname -m)" ;;
*) record FAIL "host architecture is x86-64" "$(uname -m)" ;;
esac

echo
echo '### 2. Kernel release versus REQUIRED_KERNEL_FEATURES'
python3 - "$GATE" <<'PY'
import importlib.util, sys
spec = importlib.util.spec_from_file_location("gate", sys.argv[1])
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
features = gate.observe_kernel_features()
bad = 0
for name in gate.REQUIRED_KERNEL_FEATURES:
    value = features.get(name)
    state = "PASS" if value is True else "FAIL"
    if value is not True:
        bad = 1
    print(f"[{state}] kernel feature {name} ({gate.KERNEL_FEATURE_BASIS[name]}) -- {value!r}")
sys.exit(bad)
PY
if [ "$?" -eq 0 ]; then
	record PASS "all 12 REQUIRED_KERNEL_FEATURES observed present" "see lines above"
else
	record FAIL "all 12 REQUIRED_KERNEL_FEATURES observed present" "see lines above"
fi

echo
echo '### 3. Unprivileged user namespaces'
knob=/proc/sys/kernel/unprivileged_userns_clone
if [ -r "$knob" ] && [ "$(cat "$knob")" != "1" ]; then
	record FAIL "unprivileged_userns_clone permits userns" "$(cat "$knob")"
else
	record PASS "unprivileged_userns_clone does not forbid userns" "$([ -r "$knob" ] && cat "$knob" || echo 'knob absent')"
fi
aa=/proc/sys/kernel/apparmor_restrict_unprivileged_userns
record PASS "apparmor_restrict_unprivileged_userns observed" "$([ -r "$aa" ] && cat "$aa" || echo 'knob absent')"
check "unshare(1) is installed" "$(command -v unshare 2>/dev/null)" command -v unshare
check "unprivileged user namespace can be created" "unshare --user --map-root-user true succeeded" \
	unshare --user --map-root-user true
check "full wrapper namespace set can be created" "unshare --user --map-root-user --mount --net --ipc --uts succeeded" \
	unshare --user --map-root-user --mount --net --ipc --uts true

echo
echo '### 4. cgroup-v2 unified hierarchy'
if [ -e /sys/fs/cgroup/cgroup.controllers ]; then
	record PASS "cgroup-v2 unified hierarchy at /sys/fs/cgroup" "$(stat -f -c '%T' /sys/fs/cgroup)"
else
	record FAIL "cgroup-v2 unified hierarchy at /sys/fs/cgroup" "cgroup.controllers absent"
fi
available="$(cat /sys/fs/cgroup/cgroup.controllers 2>/dev/null || echo '')"
for controller in cpu memory pids; do
	case " ${available} " in
	*" ${controller} "*) record PASS "root exposes the ${controller} controller" "${available}" ;;
	*) record FAIL "root exposes the ${controller} controller" "${available}" ;;
	esac
done

echo
echo '### 5. Delegation to the unprivileged runner user (wrapper default parent)'
# The wrapper defaults SEMAPRAX_DOCTOR_GATE_PARENT to user.slice and requires
# that this user may write its cgroup.subtree_control.
if [ -w /sys/fs/cgroup/user.slice/cgroup.subtree_control ]; then
	record PASS "user.slice/cgroup.subtree_control writable unprivileged" "writable"
else
	record FAIL "user.slice/cgroup.subtree_control writable unprivileged" \
		"not writable as $(id -un); wrapper needs SEMAPRAX_DOCTOR_GATE_PARENT"
fi

echo
echo '### 6. Delegation via a root-created, chowned parent'
PARENT=/sys/fs/cgroup/semaprax-doctor-probe
SCOPE="${PARENT}/scope.$$"
if sudo -n true 2>/dev/null; then
	record PASS "passwordless sudo available to provision a parent" "yes"
	sudo sh -c "echo '+cpu +memory +pids' > /sys/fs/cgroup/cgroup.subtree_control" 2>&1 &&
		record PASS "root subtree_control accepts +cpu +memory +pids" "$(cat /sys/fs/cgroup/cgroup.subtree_control)" ||
		record FAIL "root subtree_control accepts +cpu +memory +pids" "write refused"
	sudo mkdir -p "${PARENT}" &&
		sudo chown -R "$(id -un):$(id -gn)" "${PARENT}" &&
		record PASS "delegated parent created and chowned" "${PARENT}" ||
		record FAIL "delegated parent created and chowned" "${PARENT}"
	ok=1
	for controller in cpu memory pids; do
		printf '+%s\n' "${controller}" >"${PARENT}/cgroup.subtree_control" 2>/dev/null || ok=0
	done
	[ "${ok}" = "1" ] &&
		record PASS "unprivileged write of +cpu +memory +pids to the delegated parent" \
			"$(cat "${PARENT}/cgroup.subtree_control" 2>/dev/null)" ||
		record FAIL "unprivileged write of +cpu +memory +pids to the delegated parent" "refused"
	mkdir "${SCOPE}" 2>/dev/null &&
		record PASS "unprivileged creation of the delegated scope" "${SCOPE}" ||
		record FAIL "unprivileged creation of the delegated scope" "${SCOPE}"
else
	record FAIL "passwordless sudo available to provision a parent" "no"
fi

echo
echo '### 7. REQUIRED_CGROUP_FILES and REQUIRED_CGROUP_WRITABLE in the scope'
if [ -d "${SCOPE}" ]; then
	for name in cgroup.controllers cgroup.events cgroup.kill cgroup.procs \
		cgroup.subtree_control cpu.max memory.max pids.max; do
		[ -e "${SCOPE}/${name}" ] &&
			record PASS "scope exposes ${name}" "present" ||
			record FAIL "scope exposes ${name}" "absent"
	done
	for name in cgroup.kill cgroup.procs cpu.max memory.max pids.max; do
		[ -w "${SCOPE}/${name}" ] &&
			record PASS "scope can write ${name}" "writable" ||
			record FAIL "scope can write ${name}" "not writable"
	done
	populated="$(sed -n 's/^populated //p' "${SCOPE}/cgroup.events" 2>/dev/null)"
	[ "${populated}" = "0" ] &&
		record PASS "scope reports populated 0" "${populated}" ||
		record FAIL "scope reports populated 0" "${populated:-unobserved}"
	procs="$(cat "${SCOPE}/cgroup.procs" 2>/dev/null | tr '\n' ' ')"
	[ -z "${procs// /}" ] &&
		record PASS "scope is empty" "cgroup.procs empty" ||
		record FAIL "scope is empty" "${procs}"
else
	record FAIL "delegated scope exists for the control-file probe" "no scope"
fi

echo
echo '### 8. The gate itself, provisioned as far as this probe honestly can'
# Everything below is real: a real delegated scope, real namespaces, real
# acknowledgement values from the wrapper. What is deliberately absent is the
# sealed current-head release, which this probe does not build. The remaining
# failures therefore say exactly which requirements are release-provisioning
# rather than host-capability.
export SEMAPRAX_DOCTOR_GATE_CGROUP="${SCOPE}"
export SEMAPRAX_DOCTOR_GATE_DISPOSABLE=yes
export SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT=private-mapped-user-mount-clean-worker-cgroup-v1
export SEMAPRAX_DOCTOR_ROOT_TEST_CONTEXT=private-user-mount-v1
unshare --user --map-root-user --mount --net --ipc --uts -- \
	python3 "${GATE}" --evidence "${RUNNER_TEMP:-/tmp}/probe-evidence.json"
echo "gate exit: $?"

echo
echo '### Summary'
printf '%s\n' "${RESULTS[@]}" | awk -F'|' '{printf "%-5s %s\n", $1, $2}'
echo
if [ "${FAILED}" -eq 0 ]; then
	echo 'Every probed HOST requirement is satisfiable on this GitHub-hosted runner.'
	echo 'This is NOT a gate result: no lifecycle fixture ran and no sealed image was built.'
else
	echo 'At least one host requirement FAILED on this GitHub-hosted runner (see the table).'
fi
exit "${FAILED}"
