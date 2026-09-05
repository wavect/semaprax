#!/usr/bin/env python3
"""Run the private Linux offline doctor lifecycle gates, or fail closed.

The lifecycle fixtures this gate selects are `#[ignore]`d in the tree because
they need a provisioned host: Linux x86-64, a private mapped user/mount
namespace, an empty delegated cgroup-v2 scope, executable sealed memfds, and
immutable current-head launcher/worker/collector images. `AGENTS.md` and
`docs/DOCTOR-PRODUCTION-PROVISIONER-V1.md` both require that missing
namespace, cgroup, sealing, or kernel prerequisites *fail* rather than skip,
because a skip that reads as green is indistinguishable from a passing
confinement result.

So this script has no skip path and no partial-credit path. Every reason it
cannot run is a nonzero exit, and every reason the harness did not actually
execute the named tests -- a filtered selection, a still-ignored test, a suite
that matched nothing -- is likewise a nonzero exit.

The decision logic is pure: `precondition_failures`, `libtest_failures`, and
`settlement_failures` take plain dictionaries and strings, so `--self-test`
drives them with synthetic inputs on any host, including the macOS and
Windows developer machines that can never run the gate itself. `--self-test`
is not evidence that the gate passed; it is evidence that the gate refuses.

    python3 scripts/doctor-provisioned-linux-gate.py --self-test
    python3 scripts/doctor-provisioned-linux-gate.py --plan
    python3 scripts/doctor-provisioned-linux-gate.py --evidence out.json

`docs/DOCTOR-PROVISIONED-LINUX-GATE-V1.md` owns the provisioning procedure and
states, unambiguously, that the gate has never been executed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import time

# --------------------------------------------------------------------------
# Admitted host, kernel, cgroup and fixture inventory
# --------------------------------------------------------------------------

# The owning contract admits Linux x86-64 and AArch64. This gate admits only
# x86-64: issue #61 scopes one environment, and AArch64, Windows and macOS
# stay separately tracked. Widening this tuple is a contract change, not a
# convenience.
ADMITTED_MACHINE = "x86_64"
ADMITTED_SYSTEM = "Linux"
ADMITTED_POINTER_BITS = 64
ADMITTED_BYTE_ORDER = "little"

# Every feature the provisioner and the selected fixtures depend on. Each must
# be observed present; `None` (unknown) is a rejection, not a pass.
REQUIRED_KERNEL_FEATURES = (
    "clone3",
    "clone_into_cgroup",
    "close_range",
    "memfd_exec",
    "memfd_sealing",
    "no_new_privs",
    "openat2",
    "pidfd_open",
    "pidfd_send_signal",
    "pivot_root",
    "seccomp_filter",
    "unprivileged_user_namespaces",
)

# The aggregate limits the provisioner installs and rereads.
REQUIRED_CGROUP_CONTROLLERS = ("cpu", "memory", "pids")

# The exact control-file inventory descriptor 9 must expose.
REQUIRED_CGROUP_FILES = (
    "cgroup.controllers",
    "cgroup.events",
    "cgroup.kill",
    "cgroup.procs",
    "cgroup.subtree_control",
    "cpu.max",
    "memory.max",
    "pids.max",
)

# The fixed writes the delegated scope must accept.
REQUIRED_CGROUP_WRITABLE = (
    "cgroup.kill",
    "cgroup.procs",
    "cpu.max",
    "memory.max",
    "pids.max",
)

# Context acknowledgements. These are provisioner assertions, never proof; the
# script still checks every property it can observe independently.
REQUIRED_CONTEXT_ENVIRONMENT = {
    "SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT": (
        "private-mapped-user-mount-clean-worker-cgroup-v1"
    ),
    "SEMAPRAX_DOCTOR_ROOT_TEST_CONTEXT": "private-user-mount-v1",
}

# Immutable current-head images, by the variable each fixture reads.
REQUIRED_IMAGE_ENVIRONMENT = (
    "SEMAPRAX_DOCTOR_COLLECTOR",
    "SEMAPRAX_DOCTOR_LAUNCHER",
    "SEMAPRAX_DOCTOR_WORKER",
)

# The real-distribution fixture's independent expectations.
REQUIRED_FIXTURE_ENVIRONMENT = (
    "SEMAPRAX_DOCTOR_EXPECTED_CLANG_DETAIL",
    "SEMAPRAX_DOCTOR_EXPECTED_NODE_DETAIL",
    "SEMAPRAX_DOCTOR_EXPECTED_RUST_DETAIL",
    "SEMAPRAX_DOCTOR_REAL_BUNDLE",
    "SEMAPRAX_DOCTOR_REAL_SELECTOR",
)

# Disposable fixtures are signed under the test-only anchor. A production
# release key must never reach this gate's environment or its evidence.
ADMITTED_TRUST_ANCHOR = "test-only"

# The collector harness: healthy materialize/execute/settle, forged,
# cross-bound and truncated replies, cancellation and deadlines, descendant
# survival, output overflow, and failure during bootstrap.
COLLECTOR_TESTS = (
    "actual_worker_materializes_executes_and_settles_before_canonical_report",
    "complete_frame_and_capture_eof_each_still_require_worker_exit",
    "complete_literal_frame_followed_by_nonzero_exit_never_becomes_a_report",
    "created_handoff::"
    "production_created_native_and_all_files_reach_worker_and_reject_digest_drift",
    "launched_handoff::"
    "production_launcher_rejects_both_image_defects_and_digest_drift",
    "launched_handoff::"
    "production_launcher_rejects_structural_collector_with_missing_loader",
    "launched_handoff::"
    "production_launcher_reports_native_and_all_from_literal_transport_files",
    "literal_reply_surrogates_reject_cross_binding_and_malformed_frames",
    "nonchild::"
    "nonchild_pidfd_rejects_without_killing_or_stopping_the_owned_sentinel",
    "physical_reports::"
    "all_three_roles_settle_and_tool_failure_is_an_ordinary_exit_one_report",
    "physical_reports::"
    "closed_report_sink_fails_after_collection_without_successful_delivery",
    "prepared_handoff::"
    "prepared_native_and_all_role_handoffs_preserve_literal_wire_and_reject_"
    "transport_drift",
    "real_launched_handoff::"
    "production_launcher_reports_all_roles_from_provisioned_real_distributions",
)

# The platform-sys lib harness: worker policy, hostile authority routes,
# supervisor/child lifecycle, and detached read-only root materialization.
PLATFORM_TESTS = (
    "doctor::offline_root::linux::tests::"
    "provisioned_close_uncertainty_is_fail_stop",
    "doctor::offline_root::linux::tests::"
    "provisioned_detached_root_bytes_modes_and_read_only",
    "doctor::offline_root::linux::tests::"
    "provisioned_metadata_mismatches_feed_actual_admission",
    "doctor::offline_root::linux::tests::"
    "provisioned_setup_and_exact_write_failures_return_no_root",
    "doctor::offline_root::linux::tests::"
    "provisioned_wrong_page_cost_stops_before_tree_writes",
    "doctor::offline_worker::tests::hostile::"
    "provisioned_capability_operations_and_process_creation_are_denied",
    "doctor::offline_worker::tests::hostile::"
    "provisioned_root_hides_real_outside_file_and_rejects_write_opens",
    "doctor::offline_worker::tests::hostile::"
    "provisioned_stdin_is_eof_and_nonstandard_descriptors_are_closed",
    "doctor::offline_worker::tests::lifecycle::"
    "post_exec_capabilities_and_supervisor_death_are_observed_externally",
    "doctor::offline_worker::tests::"
    "provisioned_materializer_exec_and_socket_denial",
    "doctor::offline_worker::tests::"
    "provisioned_missing_role_bad_hash_and_invalid_request_emit_no_frame",
    "doctor::offline_worker::tests::"
    "provisioned_overflow_and_timeout_publish_only_settled_failure",
    "doctor::offline_worker::tests::"
    "provisioned_real_clang_node_rust_distributions",
)

# Each suite runs serially: the fixtures reserve fixed descriptor destinations
# and must not compete with another descriptor mutator in the same process.
SUITES = (
    {
        "id": "collector-provisioned",
        "package": "semaprax-doctor-collector",
        "selector": ["--test", "provisioned"],
        "tests": COLLECTOR_TESTS,
    },
    {
        "id": "platform-sys-lib",
        "package": "semaprax-native-rust-interop-platform-sys",
        "selector": ["--lib"],
        "tests": PLATFORM_TESTS,
    },
)

# Bounded capture. Overflow is recorded, never silently dropped, and never a
# reason to rerun under a wider bound.
CAPTURE_LIMIT = 4 * 1024 * 1024

# The two 60s deadline fixtures dominate; the rest are seconds.
SUITE_TIMEOUT_SECONDS = 3600


# --------------------------------------------------------------------------
# Pure decision logic
# --------------------------------------------------------------------------


def _absent(value):
    """A precondition the probe could not observe is a rejection."""
    return value is None


def precondition_failures(probe):
    """Every reason this host must not run the gate, in a stable order.

    `probe` is a plain dictionary of observed facts. An unobservable fact is
    `None` and rejects: the contract calls an unobservable disagreement a
    violated trusted-launch precondition, not a reason to continue.
    """
    if not isinstance(probe, dict):
        return [f"probe must be a mapping, got {type(probe).__name__}"]
    reasons = []

    host = probe.get("host")
    if not isinstance(host, dict):
        reasons.append("probe has no host facts")
    else:
        if host.get("system") != ADMITTED_SYSTEM:
            reasons.append(
                f"host system is {host.get('system')!r}, "
                f"not {ADMITTED_SYSTEM!r}"
            )
        if host.get("machine") != ADMITTED_MACHINE:
            reasons.append(
                f"host architecture is {host.get('machine')!r}, not "
                f"{ADMITTED_MACHINE!r}; AArch64, Windows and macOS are "
                "separately tracked and this gate never generalizes to them"
            )
        if host.get("pointer_bits") != ADMITTED_POINTER_BITS:
            reasons.append(
                f"host pointer width is {host.get('pointer_bits')!r}, "
                f"not {ADMITTED_POINTER_BITS}"
            )
        if host.get("byte_order") != ADMITTED_BYTE_ORDER:
            reasons.append(
                f"host byte order is {host.get('byte_order')!r}, "
                f"not {ADMITTED_BYTE_ORDER!r}"
            )
        if not host.get("kernel_release"):
            reasons.append("host kernel release was not observed")
        if host.get("disposable") is not True:
            reasons.append(
                "host is not declared a disposable trusted environment; the "
                "gate must not run over arbitrary fork input"
            )

    features = probe.get("kernel_features")
    if not isinstance(features, dict):
        reasons.append("probe has no kernel-feature facts")
    else:
        for feature in REQUIRED_KERNEL_FEATURES:
            observed = features.get(feature)
            if _absent(observed):
                reasons.append(f"kernel feature {feature} was not observed")
            elif observed is not True:
                reasons.append(f"kernel feature {feature} is unavailable")

    cgroup = probe.get("cgroup")
    if not isinstance(cgroup, dict):
        reasons.append("probe has no delegated cgroup-v2 facts")
    else:
        if cgroup.get("filesystem") != "cgroup2":
            reasons.append(
                f"invocation cgroup filesystem is "
                f"{cgroup.get('filesystem')!r}, not 'cgroup2'"
            )
        path = cgroup.get("path")
        if not isinstance(path, str) or not path.startswith("/"):
            reasons.append("invocation cgroup path is not absolute")
        if cgroup.get("delegated") is not True:
            reasons.append(
                "invocation cgroup-v2 scope is not delegated for the fixed "
                "provisioner writes"
            )
        controllers = cgroup.get("controllers")
        if not isinstance(controllers, (list, tuple, set, frozenset)):
            reasons.append("invocation cgroup controllers were not observed")
        else:
            for controller in REQUIRED_CGROUP_CONTROLLERS:
                if controller not in controllers:
                    reasons.append(
                        f"invocation cgroup lacks the {controller} controller"
                    )
        files = cgroup.get("files")
        if not isinstance(files, (list, tuple, set, frozenset)):
            reasons.append("invocation cgroup control files were not observed")
        else:
            for name in REQUIRED_CGROUP_FILES:
                if name not in files:
                    reasons.append(f"invocation cgroup lacks {name}")
        writable = cgroup.get("writable")
        if not isinstance(writable, (list, tuple, set, frozenset)):
            reasons.append("invocation cgroup writability was not observed")
        else:
            for name in REQUIRED_CGROUP_WRITABLE:
                if name not in writable:
                    reasons.append(f"invocation cgroup cannot write {name}")
        if cgroup.get("populated") != 0:
            reasons.append(
                f"invocation cgroup reports populated "
                f"{cgroup.get('populated')!r}, not 0"
            )
        procs = cgroup.get("procs")
        if procs is None:
            reasons.append("invocation cgroup membership was not observed")
        elif list(procs):
            reasons.append(
                f"invocation cgroup is not empty: {sorted(procs)!r}"
            )

    environment = probe.get("environment")
    if not isinstance(environment, dict):
        reasons.append("probe has no environment facts")
    else:
        for name, expected in sorted(REQUIRED_CONTEXT_ENVIRONMENT.items()):
            observed = environment.get(name)
            if observed != expected:
                reasons.append(
                    f"{name} is {observed!r}, not {expected!r}"
                )
        for name in REQUIRED_FIXTURE_ENVIRONMENT:
            value = environment.get(name)
            if not value:
                reasons.append(f"{name} is not provisioned")

    images = probe.get("images")
    if not isinstance(images, dict):
        reasons.append("probe has no current-head image facts")
    else:
        for name in REQUIRED_IMAGE_ENVIRONMENT:
            image = images.get(name)
            if not isinstance(image, dict):
                reasons.append(f"{name} image was not observed")
                continue
            path = image.get("path")
            if not isinstance(path, str) or not path.startswith("/"):
                reasons.append(f"{name} path is not absolute")
            if image.get("regular_file") is not True:
                reasons.append(f"{name} is not one physical regular file")
            if image.get("executable") is not True:
                reasons.append(f"{name} is not executable")
            if image.get("elf_machine") != ADMITTED_MACHINE:
                reasons.append(
                    f"{name} ELF machine is {image.get('elf_machine')!r}, "
                    f"not {ADMITTED_MACHINE!r}"
                )
            if image.get("static_elf") is not True:
                reasons.append(
                    f"{name} is not a static ELF image; a dynamic loader "
                    "cannot be reopened during held-image execution"
                )
            if image.get("interpreter") is not None:
                reasons.append(
                    f"{name} carries PT_INTERP "
                    f"{image.get('interpreter')!r}"
                )
            digest = image.get("sha256")
            if not isinstance(digest, str) or len(digest) != 64:
                reasons.append(f"{name} has no recorded SHA-256 identity")

    release = probe.get("release")
    if not isinstance(release, dict):
        reasons.append("probe has no unpacked release facts")
    else:
        root = release.get("root")
        if not isinstance(root, str) or not root.startswith("/"):
            reasons.append("release root is not an absolute path")
        if release.get("outside_checkout") is not True:
            reasons.append(
                "release archive was not unpacked outside the checkout; "
                "target-directory binaries are insufficient"
            )
        for field in ("archive_sha256", "manifest_sha256", "capsule_sha256"):
            digest = release.get(field)
            if not isinstance(digest, str) or len(digest) != 64:
                reasons.append(f"release {field} was not recorded")
        anchor = release.get("trust_anchor")
        if anchor != ADMITTED_TRUST_ANCHOR:
            reasons.append(
                f"release trust anchor is {anchor!r}, not "
                f"{ADMITTED_TRUST_ANCHOR!r}; the gate signs disposable "
                "fixtures only"
            )
        if release.get("production_signing_material_present") is not False:
            reasons.append(
                "production signing material is present; it must never reach "
                "this gate's environment or evidence"
            )

    revision = probe.get("revision")
    if not isinstance(revision, dict):
        reasons.append("probe has no revision facts")
    else:
        commit = revision.get("commit")
        if (
            not isinstance(commit, str)
            or len(commit) != 40
            or set(commit) - set("0123456789abcdef")
        ):
            reasons.append(
                f"checked-out commit is not 40 lowercase hex digits: "
                f"{commit!r}"
            )
        if revision.get("clean") is not True:
            reasons.append(
                "checkout is not clean; evidence must bind exact source bytes"
            )
        if (
            isinstance(release, dict)
            and isinstance(commit, str)
            and release.get("commit") != commit
        ):
            reasons.append(
                f"release archive was built at {release.get('commit')!r}, "
                f"not the checked-out {commit!r}"
            )

    tools = probe.get("tools")
    if not isinstance(tools, dict):
        reasons.append("probe has no tool facts")
    else:
        for tool in ("cargo", "rustc"):
            if not tools.get(tool):
                reasons.append(f"{tool} version was not observed")

    return reasons


_SUMMARY = re.compile(
    r"^test result: (?P<verdict>\S+)\. (?P<passed>\d+) passed; "
    r"(?P<failed>\d+) failed; (?P<ignored>\d+) ignored; "
    r"(?P<measured>\d+) measured; (?P<filtered>\d+) filtered out"
)
_OK = re.compile(r"^test (?P<name>\S+) \.\.\. ok\s*$")
_NOT_OK = re.compile(r"^test (?P<name>\S+) \.\.\. (?P<verdict>\S.*?)\s*$")


def libtest_failures(expected, stdout, exit_code):
    """Every reason a harness run is not proof that `expected` executed.

    A skip is the failure this function exists to catch. libtest reports a
    still-ignored test as `ignored`, an unmatched filter as `0 passed`, and a
    narrowed selection as fewer passes than requested; each is rejected here
    exactly as loudly as an assertion failure would be.
    """
    expected = list(expected)
    reasons = []
    if exit_code != 0:
        reasons.append(f"test harness exited {exit_code}")

    observed_ok = []
    observed_other = []
    for line in stdout.splitlines():
        match = _OK.match(line)
        if match:
            observed_ok.append(match.group("name"))
            continue
        match = _NOT_OK.match(line)
        if match and match.group("verdict") != "ok":
            observed_other.append((match.group("name"), match.group("verdict")))

    summaries = [
        match.groupdict()
        for match in (_SUMMARY.match(line) for line in stdout.splitlines())
        if match
    ]
    if not summaries:
        reasons.append(
            "test harness printed no `test result:` summary; the selection "
            "did not run"
        )
    for summary in summaries:
        if summary["verdict"] != "ok":
            reasons.append(
                f"test harness verdict is {summary['verdict']!r}, not 'ok'"
            )
        if int(summary["failed"]):
            reasons.append(f"{summary['failed']} test(s) failed")
        if int(summary["ignored"]):
            reasons.append(
                f"{summary['ignored']} test(s) were ignored rather than run; "
                "a skip is not a passing confinement result"
            )

    passed = sum(int(summary["passed"]) for summary in summaries)
    if passed != len(expected):
        reasons.append(
            f"{passed} test(s) passed, expected exactly {len(expected)}; "
            "a narrowed or unmatched selection is not a passing result"
        )

    for name, verdict in observed_other:
        reasons.append(f"test {name} reported {verdict!r}, not 'ok'")

    ok = set(observed_ok)
    for name in expected:
        if name not in ok:
            reasons.append(f"expected test {name} did not report ok")
    for name in sorted(ok - set(expected)):
        reasons.append(f"unexpected test {name} ran outside the selection")
    if len(observed_ok) != len(ok):
        reasons.append("a test name reported ok more than once")

    return reasons


def settlement_failures(before, after):
    """Every reason the owned cgroup did not settle empty after the run."""
    reasons = []
    if not isinstance(before, dict) or not isinstance(after, dict):
        return ["cgroup settlement was not observed"]
    if before.get("path") != after.get("path"):
        reasons.append(
            f"settlement observed cgroup {after.get('path')!r}, not the "
            f"admitted {before.get('path')!r}"
        )
    if after.get("exists") is not True:
        reasons.append(
            "invocation cgroup no longer exists; the gate never deletes it "
            "and cannot infer settlement from its absence"
        )
    if after.get("populated") != 0:
        reasons.append(
            f"invocation cgroup reports populated {after.get('populated')!r} "
            "after the run, not 0"
        )
    procs = after.get("procs")
    if procs is None:
        reasons.append("final cgroup membership was not observed")
    elif list(procs):
        reasons.append(
            f"descendants survived in the invocation cgroup: "
            f"{sorted(procs)!r}"
        )
    return reasons


class Settlement:
    """Sticky failure selection: cleanup never replaces the first failure."""

    def __init__(self):
        self._reasons = []

    def record(self, phase, reasons):
        for reason in reasons:
            self._reasons.append({"phase": phase, "reason": reason})

    @property
    def selected(self):
        """The first selected failure, or None."""
        return self._reasons[0] if self._reasons else None

    @property
    def reasons(self):
        return list(self._reasons)

    def failed(self):
        return bool(self._reasons)


# --------------------------------------------------------------------------
# Host observation
# --------------------------------------------------------------------------


def _read(path, limit=65536):
    try:
        with open(path, "rb") as handle:
            return handle.read(limit).decode("utf-8", "replace")
    except OSError:
        return None


def _kernel_version():
    release = platform.release()
    match = re.match(r"(\d+)\.(\d+)", release)
    if not match:
        return None
    return (int(match.group(1)), int(match.group(2)))


# How each kernel feature was decided. `version` means it was derived from the
# reported kernel release, which is a necessary condition, not a runtime
# capability proof: a distribution kernel can carry the version and still have
# the feature compiled out or policy-blocked. The fixtures themselves fail
# rather than skip when a syscall is unavailable, so this probe rejects early
# without ever claiming to be the authority.
KERNEL_FEATURE_BASIS = {
    "clone3": "version",
    "clone_into_cgroup": "version",
    "close_range": "version",
    "memfd_exec": "version",
    "memfd_sealing": "version",
    "no_new_privs": "version",
    "openat2": "version",
    "pidfd_open": "version",
    "pidfd_send_signal": "version",
    "pivot_root": "version",
    "seccomp_filter": "procfs",
    "unprivileged_user_namespaces": "procfs",
}


def observe_kernel_features():
    """Observe each required feature, leaving anything unknown as None."""
    version = _kernel_version()

    def at_least(major, minor):
        if version is None:
            return None
        return version >= (major, minor)

    userns = _read("/proc/sys/user/max_user_namespaces")
    unprivileged = _read("/proc/sys/kernel/unprivileged_userns_clone")
    if userns is None:
        unprivileged_userns = None
    else:
        try:
            allowed = int(userns.strip()) > 0
        except ValueError:
            allowed = None
        if allowed and unprivileged is not None:
            allowed = unprivileged.strip() == "1"
        unprivileged_userns = allowed

    return {
        "clone3": at_least(5, 3),
        "clone_into_cgroup": at_least(5, 7),
        "close_range": at_least(5, 9),
        "memfd_exec": at_least(6, 3),
        "memfd_sealing": at_least(3, 17),
        "no_new_privs": at_least(3, 5),
        "openat2": at_least(5, 6),
        "pidfd_open": at_least(5, 3),
        "pidfd_send_signal": at_least(5, 1),
        "pivot_root": at_least(2, 6),
        "seccomp_filter": (
            None
            if _read("/proc/sys/kernel/seccomp/actions_avail") is None
            else True
        ),
        "unprivileged_user_namespaces": unprivileged_userns,
    }


def observe_cgroup(path):
    """Observe the delegated cgroup-v2 scope named by `path`."""
    facts = {
        "path": path,
        "exists": os.path.isdir(path) if path else False,
        "filesystem": None,
        "delegated": None,
        "controllers": None,
        "files": None,
        "writable": None,
        "populated": None,
        "procs": None,
    }
    if not path or not facts["exists"]:
        return facts
    try:
        statfs = os.statvfs(path)
        facts["block_size"] = statfs.f_bsize
    except OSError:
        pass
    controllers = _read(os.path.join(path, "cgroup.controllers"))
    if controllers is not None:
        facts["filesystem"] = "cgroup2"
        facts["controllers"] = sorted(controllers.split())
    try:
        facts["files"] = sorted(os.listdir(path))
    except OSError:
        facts["files"] = None
    if facts["files"] is not None:
        facts["writable"] = sorted(
            name
            for name in facts["files"]
            if os.access(os.path.join(path, name), os.W_OK)
        )
        facts["delegated"] = all(
            name in facts["writable"] for name in REQUIRED_CGROUP_WRITABLE
        )
    events = _read(os.path.join(path, "cgroup.events"))
    if events is not None:
        for line in events.splitlines():
            key, _, value = line.partition(" ")
            if key == "populated":
                try:
                    facts["populated"] = int(value)
                except ValueError:
                    facts["populated"] = None
    procs = _read(os.path.join(path, "cgroup.procs"))
    if procs is not None:
        facts["procs"] = [line for line in procs.split() if line]
    return facts


def _digest(path):
    try:
        with open(path, "rb") as handle:
            return hashlib.file_digest(handle, "sha256").hexdigest()
    except OSError:
        return None


def observe_image(path):
    """Observe one held image: identity, ELF machine and loader closure."""
    facts = {
        "path": path,
        "regular_file": None,
        "executable": None,
        "size": None,
        "sha256": None,
        "elf_machine": None,
        "static_elf": None,
        "interpreter": None,
    }
    if not path:
        return facts
    facts["regular_file"] = os.path.isfile(path) and not os.path.islink(path)
    facts["executable"] = os.access(path, os.X_OK) if path else False
    if not facts["regular_file"]:
        return facts
    facts["size"] = os.path.getsize(path)
    facts["sha256"] = _digest(path)
    with open(path, "rb") as handle:
        header = handle.read(64)
        if len(header) < 64 or header[:4] != b"\x7fELF":
            return facts
        machine = int.from_bytes(header[18:20], "little")
        facts["elf_machine"] = {0x3E: "x86_64", 0xB7: "aarch64"}.get(machine)
        program_offset = int.from_bytes(header[32:40], "little")
        entry_size = int.from_bytes(header[54:56], "little")
        entry_count = int.from_bytes(header[56:58], "little")
        handle.seek(program_offset)
        table = handle.read(entry_size * entry_count)
    interpreter = None
    for index in range(entry_count):
        entry = table[index * entry_size : (index + 1) * entry_size]
        if len(entry) < 8:
            break
        if int.from_bytes(entry[:4], "little") == 3:  # PT_INTERP
            interpreter = "present"
    facts["interpreter"] = interpreter
    facts["static_elf"] = interpreter is None
    return facts


def _command(argv, cwd=None):
    try:
        finished = subprocess.run(
            argv,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if finished.returncode != 0:
        return None
    return finished.stdout.strip()


def observe(root, arguments):
    """Build the probe this gate decides on."""
    environment = dict(os.environ)
    cgroup_path = environment.get("SEMAPRAX_DOCTOR_GATE_CGROUP")
    commit = _command(["git", "rev-parse", "HEAD"], cwd=root)
    status = _command(["git", "status", "--porcelain"], cwd=root)
    return {
        "host": {
            "system": platform.system(),
            "machine": platform.machine(),
            "pointer_bits": 64 if sys.maxsize > 2**32 else 32,
            "byte_order": sys.byteorder,
            "kernel_release": platform.release(),
            "kernel_version": platform.version(),
            "disposable": environment.get("SEMAPRAX_DOCTOR_GATE_DISPOSABLE")
            == "yes",
        },
        "kernel_features": observe_kernel_features(),
        "kernel_feature_basis": dict(KERNEL_FEATURE_BASIS),
        "cgroup": observe_cgroup(cgroup_path),
        "environment": {
            name: environment.get(name)
            for name in sorted(
                set(REQUIRED_CONTEXT_ENVIRONMENT)
                | set(REQUIRED_FIXTURE_ENVIRONMENT)
                | set(REQUIRED_IMAGE_ENVIRONMENT)
            )
        },
        "images": {
            name: observe_image(environment.get(name))
            for name in REQUIRED_IMAGE_ENVIRONMENT
        },
        "release": {
            "root": environment.get("SEMAPRAX_DOCTOR_GATE_RELEASE_ROOT"),
            "outside_checkout": _outside(
                root, environment.get("SEMAPRAX_DOCTOR_GATE_RELEASE_ROOT")
            ),
            "archive_sha256": _digest(
                environment.get("SEMAPRAX_DOCTOR_GATE_RELEASE_ARCHIVE") or ""
            ),
            "manifest_sha256": _digest(
                environment.get("SEMAPRAX_DOCTOR_GATE_RELEASE_MANIFEST") or ""
            ),
            "capsule_sha256": _digest(
                environment.get("SEMAPRAX_DOCTOR_GATE_RELEASE_CAPSULE") or ""
            ),
            "commit": environment.get("SEMAPRAX_DOCTOR_GATE_RELEASE_COMMIT"),
            "trust_anchor": environment.get(
                "SEMAPRAX_DOCTOR_GATE_TRUST_ANCHOR"
            ),
            "production_signing_material_present": bool(
                environment.get("SEMAPRAX_DOCTOR_RELEASE_SIGNING_KEY")
                or environment.get("SEMAPRAX_DOCTOR_RELEASE_PRIVATE_KEY_HEX")
            ),
        },
        "revision": {
            "commit": commit,
            "clean": status == "" if status is not None else None,
        },
        "tools": {
            "cargo": _command(["cargo", "-Vv"]),
            "rustc": _command(["rustc", "-vV"]),
            "cargo_path": shutil.which("cargo"),
            "rustc_path": shutil.which("rustc"),
        },
        "fixture": {
            "bundle": environment.get("SEMAPRAX_DOCTOR_REAL_BUNDLE"),
            "bundle_sha256": _digest(
                environment.get("SEMAPRAX_DOCTOR_REAL_BUNDLE") or ""
            ),
            "selector": environment.get("SEMAPRAX_DOCTOR_REAL_SELECTOR"),
        },
        "gate": {
            "argv": list(arguments),
            "started": time.time(),
        },
    }


def _outside(root, candidate):
    if not candidate:
        return None
    try:
        real_root = os.path.realpath(root)
        real_candidate = os.path.realpath(candidate)
    except OSError:
        return None
    return not (
        real_candidate == real_root
        or real_candidate.startswith(real_root + os.sep)
    )


# --------------------------------------------------------------------------
# Execution
# --------------------------------------------------------------------------


def suite_command(suite):
    """The exact, explicit, serial selection for one suite."""
    argv = [
        "cargo",
        "test",
        "--locked",
        "--offline",
        "-p",
        suite["package"],
        *suite["selector"],
        "--",
        "--ignored",
        "--exact",
        "--test-threads=1",
    ]
    argv.extend(suite["tests"])
    return argv


def _bounded(stream):
    raw = stream.encode("utf-8", "replace")
    return {
        "bytes": len(raw),
        "sha256": hashlib.sha256(raw).hexdigest(),
        "truncated": len(raw) > CAPTURE_LIMIT,
        "text": raw[:CAPTURE_LIMIT].decode("utf-8", "replace"),
    }


def run_suite(root, suite):
    argv = suite_command(suite)
    started = time.time()
    try:
        finished = subprocess.run(
            argv,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=SUITE_TIMEOUT_SECONDS,
            check=False,
        )
        stdout, stderr, code = (
            finished.stdout,
            finished.stderr,
            finished.returncode,
        )
    except subprocess.TimeoutExpired as expired:
        stdout = expired.stdout or ""
        stderr = (expired.stderr or "") + "\ngate: suite deadline expired"
        code = 124
    except OSError as error:
        stdout, stderr, code = "", f"gate: cannot start cargo: {error}", 127
    return {
        "id": suite["id"],
        "command": argv,
        "exit_code": code,
        "elapsed_seconds": round(time.time() - started, 3),
        "expected_tests": list(suite["tests"]),
        "stdout": _bounded(stdout),
        "stderr": _bounded(stderr),
        "failures": libtest_failures(suite["tests"], stdout, code),
    }


def execute(root, probe, evidence_path):
    settlement = Settlement()
    settlement.record("selection", selection_drift(root))
    settlement.record("precondition", precondition_failures(probe))

    suites = []
    if not settlement.failed():
        for suite in SUITES:
            observed = run_suite(root, suite)
            suites.append(observed)
            settlement.record(f"execute:{suite['id']}", observed["failures"])
            if observed["failures"]:
                # Failure selection is sticky. Later suites are not run, and
                # cleanup below cannot replace this selected status.
                break

    # Cleanup and settlement observation run whether or not execution failed;
    # their findings are appended, never promoted over the first failure.
    after = observe_cgroup(probe.get("cgroup", {}).get("path"))
    settlement.record(
        "settle", settlement_failures(probe.get("cgroup", {}), after)
    )

    evidence = {
        "schema": "semaprax.doctor.provisioned-linux-gate.v1",
        "generated": time.time(),
        "probe": probe,
        "final_cgroup": after,
        "suites": suites,
        "selected_failure": settlement.selected,
        "failures": settlement.reasons,
        "verdict": "failed" if settlement.failed() else "passed",
    }
    if evidence_path:
        with open(evidence_path, "w", encoding="utf-8") as handle:
            json.dump(evidence, handle, indent=2, sort_keys=True)
            handle.write("\n")
    return evidence, settlement


# --------------------------------------------------------------------------
# Self-test: the gate's refusal, proved without a Linux host
# --------------------------------------------------------------------------


def _admissible_probe():
    """A synthetic probe that satisfies every precondition."""
    return {
        "host": {
            "system": "Linux",
            "machine": "x86_64",
            "pointer_bits": 64,
            "byte_order": "little",
            "kernel_release": "6.8.0-generic",
            "disposable": True,
        },
        "kernel_features": {name: True for name in REQUIRED_KERNEL_FEATURES},
        "cgroup": {
            "path": "/sys/fs/cgroup/semaprax-doctor.scope",
            "exists": True,
            "filesystem": "cgroup2",
            "delegated": True,
            "controllers": list(REQUIRED_CGROUP_CONTROLLERS),
            "files": list(REQUIRED_CGROUP_FILES),
            "writable": list(REQUIRED_CGROUP_WRITABLE),
            "populated": 0,
            "procs": [],
        },
        "environment": {
            **REQUIRED_CONTEXT_ENVIRONMENT,
            "SEMAPRAX_DOCTOR_REAL_BUNDLE": "/opt/spx/bundle.bin",
            "SEMAPRAX_DOCTOR_REAL_SELECTOR": "release-fixture",
            "SEMAPRAX_DOCTOR_EXPECTED_CLANG_DETAIL": "/bin/clang (clang 20)",
            "SEMAPRAX_DOCTOR_EXPECTED_NODE_DETAIL": "v22.11.0",
            "SEMAPRAX_DOCTOR_EXPECTED_RUST_DETAIL": "rustc 1.88.0",
        },
        "images": {
            name: {
                "path": f"/opt/spx/{name.lower()}",
                "regular_file": True,
                "executable": True,
                "elf_machine": "x86_64",
                "static_elf": True,
                "interpreter": None,
                "sha256": "a" * 64,
            }
            for name in REQUIRED_IMAGE_ENVIRONMENT
        },
        "release": {
            "root": "/opt/spx/release",
            "outside_checkout": True,
            "archive_sha256": "b" * 64,
            "manifest_sha256": "c" * 64,
            "capsule_sha256": "d" * 64,
            "commit": "e" * 40,
            "trust_anchor": ADMITTED_TRUST_ANCHOR,
            "production_signing_material_present": False,
        },
        "revision": {"commit": "e" * 40, "clean": True},
        "tools": {"cargo": "cargo 1.88.0", "rustc": "rustc 1.88.0"},
    }


def _mutate(probe, path, value):
    """Copy `probe` with one nested key replaced."""
    clone = {key: dict(inner) for key, inner in probe.items()}
    section, key = path
    clone[section] = dict(clone[section])
    if value is _DROP:
        clone[section].pop(key, None)
    else:
        clone[section][key] = value
    return clone


class _Drop:
    pass


_DROP = _Drop()


def _passing_output(names):
    lines = [f"running {len(names)} tests"]
    lines.extend(f"test {name} ... ok" for name in names)
    lines.append(
        f"test result: ok. {len(names)} passed; 0 failed; 0 ignored; "
        "0 measured; 0 filtered out; finished in 1.00s"
    )
    return "\n".join(lines) + "\n"


def self_test(root=None):
    """Drive the pure decision logic with synthetic inputs. Returns 0 or 1."""
    checks = []

    def check(name, condition, detail=""):
        checks.append((name, bool(condition), detail))

    good = _admissible_probe()
    check(
        "an admissible probe is admitted",
        precondition_failures(good) == [],
        repr(precondition_failures(good)),
    )

    # 1. A missing kernel feature rejects -- present, absent and unknown.
    for feature in REQUIRED_KERNEL_FEATURES:
        for value, expected in ((False, "unavailable"), (None, "not observed")):
            probe = {**good, "kernel_features": {**good["kernel_features"]}}
            probe["kernel_features"][feature] = value
            reasons = precondition_failures(probe)
            check(
                f"kernel feature {feature}={value!r} rejects",
                any(feature in reason and expected in reason
                    for reason in reasons),
                repr(reasons),
            )

    # 2. Absent cgroup-v2 delegation rejects, in each observable form.
    for path, value, needle in (
        (("cgroup", "delegated"), False, "not delegated"),
        (("cgroup", "delegated"), None, "not delegated"),
        (("cgroup", "filesystem"), "cgroup", "not 'cgroup2'"),
        (("cgroup", "filesystem"), None, "not 'cgroup2'"),
        (("cgroup", "writable"), ["cgroup.procs"], "cannot write cgroup.kill"),
        (("cgroup", "controllers"), ["cpu", "pids"], "lacks the memory"),
        (("cgroup", "files"), ["cgroup.procs"], "lacks cgroup.kill"),
        (("cgroup", "populated"), 1, "populated 1"),
        (("cgroup", "procs"), ["4242"], "not empty"),
        (("cgroup", "path"), "relative/scope", "not absolute"),
    ):
        reasons = precondition_failures(_mutate(good, path, value))
        check(
            f"cgroup {path[1]}={value!r} rejects",
            any(needle in reason for reason in reasons),
            repr(reasons),
        )
    check(
        "a probe with no cgroup facts rejects",
        any(
            "no delegated cgroup-v2 facts" in reason
            for reason in precondition_failures({**good, "cgroup": None})
        ),
    )

    # 3. The wrong architecture rejects, and never silently generalizes.
    for machine in ("aarch64", "arm64", "i686", "riscv64", None):
        reasons = precondition_failures(_mutate(good, ("host", "machine"), machine))
        check(
            f"architecture {machine!r} rejects",
            any("separately tracked" in reason for reason in reasons),
            repr(reasons),
        )
    for system in ("Darwin", "Windows", None):
        reasons = precondition_failures(_mutate(good, ("host", "system"), system))
        check(
            f"system {system!r} rejects",
            any("not 'Linux'" in reason for reason in reasons),
            repr(reasons),
        )
    check(
        "a 32-bit host rejects",
        precondition_failures(_mutate(good, ("host", "pointer_bits"), 32)) != [],
    )
    check(
        "a big-endian host rejects",
        precondition_failures(_mutate(good, ("host", "byte_order"), "big")) != [],
    )
    check(
        "a host not declared disposable rejects",
        any(
            "disposable" in reason
            for reason in precondition_failures(
                _mutate(good, ("host", "disposable"), False)
            )
        ),
    )

    # 4. A missing or defective binary rejects.
    for name in REQUIRED_IMAGE_ENVIRONMENT:
        for field, value, needle in (
            ("regular_file", False, "not one physical regular file"),
            ("regular_file", None, "not one physical regular file"),
            ("executable", False, "not executable"),
            ("path", "relative/worker", "not absolute"),
            ("path", None, "not absolute"),
            ("elf_machine", "aarch64", "ELF machine"),
            ("static_elf", False, "not a static ELF"),
            ("interpreter", "present", "PT_INTERP"),
            ("sha256", None, "no recorded SHA-256"),
        ):
            probe = {**good, "images": {**good["images"]}}
            probe["images"][name] = {**good["images"][name], field: value}
            reasons = precondition_failures(probe)
            check(
                f"{name} {field}={value!r} rejects",
                any(name in reason and needle in reason for reason in reasons),
                repr(reasons),
            )
        probe = {**good, "images": {**good["images"]}}
        del probe["images"][name]
        check(
            f"an entirely absent {name} rejects",
            any(
                f"{name} image was not observed" in reason
                for reason in precondition_failures(probe)
            ),
        )

    # 5. Context, fixture, release, revision and tool preconditions.
    for name, expected in REQUIRED_CONTEXT_ENVIRONMENT.items():
        for value in (None, "", "yes", expected + "x"):
            reasons = precondition_failures(
                _mutate(good, ("environment", name), value)
            )
            check(
                f"{name}={value!r} rejects",
                any(name in reason for reason in reasons),
                repr(reasons),
            )
    for name in REQUIRED_FIXTURE_ENVIRONMENT:
        reasons = precondition_failures(_mutate(good, ("environment", name), None))
        check(
            f"unprovisioned {name} rejects",
            any(f"{name} is not provisioned" in reason for reason in reasons),
        )
    for path, value, needle in (
        (("release", "outside_checkout"), False, "outside the checkout"),
        (("release", "outside_checkout"), None, "outside the checkout"),
        (("release", "archive_sha256"), None, "archive_sha256"),
        (("release", "manifest_sha256"), "short", "manifest_sha256"),
        (("release", "capsule_sha256"), None, "capsule_sha256"),
        (("release", "root"), "opt/spx", "not an absolute path"),
        (("release", "trust_anchor"), "production", "disposable fixtures only"),
        (("release", "trust_anchor"), None, "disposable fixtures only"),
        (
            ("release", "production_signing_material_present"),
            True,
            "production signing material",
        ),
        (("release", "commit"), "f" * 40, "not the checked-out"),
        (("revision", "commit"), "HEAD", "40 lowercase hex"),
        (("revision", "commit"), None, "40 lowercase hex"),
        (("revision", "clean"), False, "not clean"),
        (("revision", "clean"), None, "not clean"),
        (("tools", "cargo"), None, "cargo version"),
        (("tools", "rustc"), None, "rustc version"),
    ):
        reasons = precondition_failures(_mutate(good, path, value))
        check(
            f"{path[0]}.{path[1]}={value!r} rejects",
            any(needle in reason for reason in reasons),
            repr(reasons),
        )
    check(
        "a non-mapping probe rejects",
        precondition_failures([]) != [] and precondition_failures(None) != [],
    )
    check(
        "an empty probe rejects on every section",
        len(precondition_failures({})) >= 7,
        repr(precondition_failures({})),
    )

    # 6. A skipped-rather-than-run outcome is a failure, not a pass.
    names = ["alpha", "beta::gamma"]
    check(
        "a genuine pass is accepted",
        libtest_failures(names, _passing_output(names), 0) == [],
        repr(libtest_failures(names, _passing_output(names), 0)),
    )
    ignored = (
        "running 2 tests\n"
        "test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; "
        "0 filtered out; finished in 0.00s\n"
    )
    reasons = libtest_failures(names, ignored, 0)
    check(
        "tests left ignored are rejected even at exit 0",
        any("ignored rather than run" in reason for reason in reasons)
        and any("0 test(s) passed" in reason for reason in reasons),
        repr(reasons),
    )
    filtered = (
        "running 0 tests\n"
        "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; "
        "13 filtered out; finished in 0.00s\n"
    )
    reasons = libtest_failures(names, filtered, 0)
    check(
        "an unmatched filter is rejected even at exit 0",
        any("expected exactly 2" in reason for reason in reasons),
        repr(reasons),
    )
    narrowed = _passing_output(names[:1])
    reasons = libtest_failures(names, narrowed, 0)
    check(
        "a narrowed selection is rejected",
        any("expected exactly 2" in reason for reason in reasons)
        and any("beta::gamma did not report ok" in reason for reason in reasons),
        repr(reasons),
    )
    check(
        "empty harness output is rejected",
        any(
            "no `test result:` summary" in reason
            for reason in libtest_failures(names, "", 0)
        ),
    )
    check(
        "a nonzero harness exit is rejected",
        any(
            "exited 101" in reason
            for reason in libtest_failures(names, _passing_output(names), 101)
        ),
    )
    failed = (
        "running 2 tests\n"
        "test alpha ... ok\n"
        "test beta::gamma ... FAILED\n"
        "test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; "
        "0 filtered out; finished in 1.00s\n"
    )
    reasons = libtest_failures(names, failed, 101)
    check(
        "a real assertion failure is rejected",
        any("1 test(s) failed" in reason for reason in reasons)
        and any("reported 'FAILED'" in reason for reason in reasons),
        repr(reasons),
    )
    extra = _passing_output(names + ["delta"]).replace(
        "3 passed", "3 passed"
    )
    reasons = libtest_failures(names, extra, 0)
    check(
        "a test outside the selection is rejected",
        any("unexpected test delta" in reason for reason in reasons),
        repr(reasons),
    )
    check(
        "the real gate selection is nonempty and unique",
        len(COLLECTOR_TESTS) == len(set(COLLECTOR_TESTS)) == 13
        and len(PLATFORM_TESTS) == len(set(PLATFORM_TESTS)) == 13,
    )
    check(
        "every required kernel feature records how it was decided",
        set(KERNEL_FEATURE_BASIS) == set(REQUIRED_KERNEL_FEATURES),
        repr(sorted(set(KERNEL_FEATURE_BASIS) ^ set(REQUIRED_KERNEL_FEATURES))),
    )
    if root is not None:
        drift = selection_drift(root)
        check(
            "the selection still equals the ignored lifecycle inventory",
            drift == [],
            repr(drift),
        )
        # A synthetic narrowing must be detected, so the check above cannot
        # pass vacuously through an inventory the parser never found.
        narrowed = tuple(SUITES[0]["tests"][1:])
        original = SUITES[0]["tests"]
        SUITES[0]["tests"] = narrowed
        try:
            check(
                "a narrowed selection is detected as drift",
                any(
                    "is not selected by the gate" in reason
                    for reason in selection_drift(root)
                ),
            )
        finally:
            SUITES[0]["tests"] = original
    for suite in SUITES:
        argv = suite_command(suite)
        check(
            f"{suite['id']} selects --ignored --exact serially",
            argv[argv.index("--") + 1 :][:3]
            == ["--ignored", "--exact", "--test-threads=1"]
            and argv[-len(suite["tests"]) :] == list(suite["tests"]),
            repr(argv),
        )

    # 7. Settlement: an unsettled cgroup fails, and cleanup never replaces the
    #    first selected failure.
    before = {"path": "/sys/fs/cgroup/x", "populated": 0, "procs": []}
    after_ok = {
        "path": "/sys/fs/cgroup/x",
        "exists": True,
        "populated": 0,
        "procs": [],
    }
    check(
        "an empty settled cgroup is accepted",
        settlement_failures(before, after_ok) == [],
        repr(settlement_failures(before, after_ok)),
    )
    for mutation, needle in (
        ({"populated": 1}, "populated 1"),
        ({"populated": None}, "populated None"),
        ({"procs": ["991"]}, "descendants survived"),
        ({"procs": None}, "membership was not observed"),
        ({"exists": False}, "no longer exists"),
        ({"path": "/sys/fs/cgroup/y"}, "not the admitted"),
    ):
        reasons = settlement_failures(before, {**after_ok, **mutation})
        check(
            f"settlement rejects {mutation!r}",
            any(needle in reason for reason in reasons),
            repr(reasons),
        )
    check(
        "unobserved settlement rejects",
        settlement_failures(before, None) != [],
    )

    sticky = Settlement()
    sticky.record("execute", ["worker never settled"])
    sticky.record("settle", ["cgroup kill uncertain"])
    check(
        "the first failure stays selected when cleanup also fails",
        sticky.selected
        == {"phase": "execute", "reason": "worker never settled"}
        and len(sticky.reasons) == 2,
        repr(sticky.reasons),
    )
    empty = Settlement()
    empty.record("precondition", [])
    check("no failures means no selection", empty.selected is None
          and not empty.failed())

    failures = [(name, detail) for name, ok, detail in checks if not ok]
    for name, detail in failures:
        print(f"self-test FAILED: {name}: {detail}", file=sys.stderr)
    print(
        f"self-test: {len(checks) - len(failures)}/{len(checks)} checks passed"
    )
    if failures:
        return 1
    print(
        "self-test proves the gate refuses; it is NOT evidence that the "
        "provisioned Linux gate has ever run."
    )
    return 0


# --------------------------------------------------------------------------
# Entry point
# --------------------------------------------------------------------------


_IGNORED_FN = re.compile(
    r"#\[ignore[^\]]*\]\s*\n(?:#\[[^\]]*\]\s*\n)*fn (\w+)"
)

# Where the gate's selection comes from, and the module prefix each file
# contributes. `SOURCE_OF_TRUTH` lets `--self-test` prove the selection still
# equals the ignored lifecycle inventory, so adding an ignored case to an owning
# harness without adding it here is a local failure rather than silent
# narrowing.
COLLECTOR_ROOT = "crates/semaprax-doctor-collector/tests"
PLATFORM_ROOT = "crates/semaprax-native-rust-interop-platform-sys/src"
SOURCE_OF_TRUTH = {
    "collector-provisioned": (
        (f"{COLLECTOR_ROOT}/provisioned.rs", ""),
        (f"{COLLECTOR_ROOT}/support/created_handoff.rs", "created_handoff"),
        (f"{COLLECTOR_ROOT}/support/launched_handoff.rs", "launched_handoff"),
        (f"{COLLECTOR_ROOT}/support/nonchild.rs", "nonchild"),
        (f"{COLLECTOR_ROOT}/support/physical_reports.rs", "physical_reports"),
        (f"{COLLECTOR_ROOT}/support/prepared_handoff.rs", "prepared_handoff"),
        (
            f"{COLLECTOR_ROOT}/support/real_launched_handoff.rs",
            "real_launched_handoff",
        ),
    ),
    "platform-sys-lib": (
        (
            f"{PLATFORM_ROOT}/doctor/offline_worker/tests.rs",
            "doctor::offline_worker::tests",
        ),
        (
            f"{PLATFORM_ROOT}/doctor/offline_worker/tests/hostile.rs",
            "doctor::offline_worker::tests::hostile",
        ),
        (
            f"{PLATFORM_ROOT}/doctor/offline_worker/tests/lifecycle.rs",
            "doctor::offline_worker::tests::lifecycle",
        ),
        (
            f"{PLATFORM_ROOT}/doctor/offline_worker/tests/native.rs",
            "doctor::offline_worker::tests::native",
        ),
        (
            f"{PLATFORM_ROOT}/doctor/offline_root/linux/tests.rs",
            "doctor::offline_root::linux::tests",
        ),
    ),
}


def selection_drift(root):
    """Every way the selection has drifted from the ignored inventory."""
    reasons = []
    for suite in SUITES:
        sources = SOURCE_OF_TRUTH[suite["id"]]
        observed = set()
        for relative, prefix in sources:
            path = os.path.join(root, relative)
            try:
                with open(path, encoding="utf-8") as handle:
                    text = handle.read()
            except OSError as error:
                reasons.append(f"cannot read {relative}: {error}")
                continue
            for name in _IGNORED_FN.findall(text):
                observed.add(f"{prefix}::{name}" if prefix else name)
        selected = set(suite["tests"])
        for name in sorted(observed - selected):
            reasons.append(
                f"{suite['id']}: ignored test {name} is not selected by the "
                "gate; a new lifecycle case must not silently narrow it"
            )
        for name in sorted(selected - observed):
            reasons.append(
                f"{suite['id']}: selected test {name} is not an ignored test "
                "in the owning harness"
            )
    return reasons


def plan():
    return {
        "schema": "semaprax.doctor.provisioned-linux-gate.plan.v1",
        "admitted_machine": ADMITTED_MACHINE,
        "required_kernel_features": list(REQUIRED_KERNEL_FEATURES),
        "required_cgroup_controllers": list(REQUIRED_CGROUP_CONTROLLERS),
        "required_cgroup_files": list(REQUIRED_CGROUP_FILES),
        "required_environment": {
            "context": dict(REQUIRED_CONTEXT_ENVIRONMENT),
            "images": list(REQUIRED_IMAGE_ENVIRONMENT),
            "fixtures": list(REQUIRED_FIXTURE_ENVIRONMENT),
        },
        "suites": [
            {
                "id": suite["id"],
                "command": suite_command(suite),
                "tests": list(suite["tests"]),
            }
            for suite in SUITES
        ],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Provisioned Linux offline doctor lifecycle gate."
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="drive the fail-closed logic with synthetic inputs and exit",
    )
    parser.add_argument(
        "--plan",
        action="store_true",
        help="print the preconditions and exact test selection, then exit",
    )
    parser.add_argument(
        "--evidence",
        help="path for the JSON evidence record this run produces",
    )
    parser.add_argument(
        "--root",
        default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        help="repository checkout to run in",
    )
    arguments = parser.parse_args(argv)

    if arguments.self_test:
        return self_test(arguments.root)
    if arguments.plan:
        json.dump(plan(), sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
        return 0

    probe = observe(arguments.root, sys.argv[1:])
    evidence, settlement = execute(arguments.root, probe, arguments.evidence)
    json.dump(
        {
            key: evidence[key]
            for key in ("schema", "verdict", "selected_failure", "failures")
        },
        sys.stdout,
        indent=2,
        sort_keys=True,
    )
    sys.stdout.write("\n")
    if settlement.failed():
        selected = settlement.selected
        print(
            "doctor provisioned Linux gate FAILED "
            f"({selected['phase']}): {selected['reason']}",
            file=sys.stderr,
        )
        for entry in settlement.reasons[1:]:
            print(
                f"  also ({entry['phase']}): {entry['reason']}",
                file=sys.stderr,
            )
        print(
            "This is a failure, not a skip. Missing provisioning is never a "
            "passing confinement result.",
            file=sys.stderr,
        )
        return 1
    print(
        "doctor provisioned Linux gate passed: "
        f"{sum(len(suite['expected_tests']) for suite in evidence['suites'])} "
        f"tests at {probe['revision']['commit']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
