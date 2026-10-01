#!/usr/bin/env python3
"""Run the explicitly provisioned Windows confinement runtime tests.

This gate fails if its Windows host or scratch parent is absent, if Cargo or
libtest fails, or if any named live test is filtered, ignored, or missing. The
runtime cases use a deterministic test-only signing key to exercise the shared
capsule verifier and held-image binding; this is not production release trust
or Windows request/bundle transport evidence.
"""

from __future__ import annotations

import argparse
import csv
import os
import platform
import re
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path

PACKAGE = "semaprax-native-rust-interop-platform-sys"
PARENT_ENV = "SEMAPRAX_WINDOWS_CONFINEMENT_TEST_PARENT"
FILTER = "windows_runtime_"
EXPECTED_TESTS = (
    "doctor::windows_confinement::primitive::tests::windows_runtime_launches_restricted_child_inside_acl_scratch_and_settles_it",
    "doctor::windows_confinement::primitive::tests::windows_runtime_timeout_terminates_the_confined_job_and_settles_cancellation",
    "doctor::windows_confinement::primitive::tests::windows_runtime_timeout_terminates_an_actual_job_descendant",
    "doctor::windows_confinement::primitive::tests::windows_runtime_nonzero_exit_settles_failed_and_cleans_resources",
    "doctor::windows_confinement::primitive::tests::windows_runtime_scratch_refusal_closes_setup_handles",
    "doctor::windows_confinement::primitive::tests::windows_runtime_signed_test_key_capsule_refusals_and_launch_settle",
    "doctor::windows_confinement::refusal::tests::windows_runtime_missing_release_anchor_refuses_before_token_job_or_filesystem",
    "doctor::windows_confinement::refusal::tests::windows_runtime_bad_signature_refuses_before_token_job_or_filesystem",
    "doctor::windows_confinement::refusal::tests::windows_runtime_signed_linux_architecture_capsule_refuses_before_token_job_or_filesystem",
    "doctor::windows_confinement::primitive::tests::windows_runtime_protected_scratch_dacl_blocks_inherited_parent_ace",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_signed_image_mismatch_refuses_before_process_effects",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_signed_image_pins_leaf_and_ancestors_through_launch",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_signed_image_refuses_preexisting_writer_and_hardlink",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_signed_image_refuses_writable_mapping_after_writer_closes",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_dropped_child_releases_image_and_process_handles",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_signed_image_refuses_retained_writable_section_without_view",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_retained_writable_section_refusals_settle_handles_and_scratch",
    "doctor::windows_confinement::primitive::tests::binding::windows_runtime_child_inherits_only_declared_standard_handles",
    "doctor::windows_confinement::carrier::tests::windows_runtime_authenticated_request_bundle_carriers_are_read_only",
    "doctor::windows_confinement::carrier::tests::windows_runtime_authenticated_carrier_rejects_invalid_artifacts_without_handles",
    "doctor::windows_confinement::carrier::tests::windows_runtime_authenticated_carrier_repeated_create_drop_settles_one_handle",
    "doctor::windows_confinement::carrier::tests::windows_runtime_authenticated_carriers_settle_independent_live_handles",
)
REPO_ROOT = Path(__file__).resolve().parents[1]
TERMINATION_TIMEOUT_SECONDS = 30
RUNTIME_TIMEOUT_SECONDS = 600


def precondition_failures(
    system,
    machine,
    pointer_bits,
    parent_present,
    parent_absolute,
    parent_directory,
    parent_reparse,
    parent_empty,
):
    failures = []
    if system != "Windows":
        failures.append("host operating system is not Windows")
    if machine not in {"AMD64", "x86_64", "ARM64", "aarch64"}:
        failures.append("host architecture is not admitted x86-64 or AArch64")
    if pointer_bits != 64:
        failures.append("host pointer width is not 64-bit")
    if not parent_present:
        failures.append(f"{PARENT_ENV} is missing")
    if not parent_absolute:
        failures.append("explicit test parent is not absolute")
    if not parent_directory:
        failures.append("explicit test parent is not an existing directory")
    if parent_reparse:
        failures.append("explicit test parent is a reparse point")
    if not parent_empty:
        failures.append("explicit test parent is not empty")
    return failures


def libtest_failures(output, return_code):
    failures = []
    output = output.replace("\r\n", "\n").replace("\r", "\n")
    if return_code != 0:
        failures.append(f"cargo test exited with status {return_code}")
    for test in EXPECTED_TESTS:
        matches = re.findall(rf"(?m)^test {re.escape(test)} \.\.\. ok$", output)
        if len(matches) != 1:
            failures.append(f"expected exactly one passing execution of {test}; saw {len(matches)}")
    summary = re.search(
        r"(?m)^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        output,
    )
    if summary is None:
        failures.append("libtest did not report a successful test summary")
    else:
        passed, failed, ignored = map(int, summary.groups())
        if passed != len(EXPECTED_TESTS) or failed != 0 or ignored != 0:
            failures.append(
                f"libtest summary must show all {len(EXPECTED_TESTS)} selected runtime tests passing "
                f"and no ignored matches; got {passed} passed, {failed} failed, {ignored} ignored"
            )
    return failures


def inspect_parent():
    value = os.environ.get(PARENT_ENV)
    if not value:
        return None, [f"{PARENT_ENV} is missing"]
    parent = Path(value)
    try:
        metadata = parent.lstat()
    except OSError as error:
        return parent, [f"explicit test parent cannot be inspected: {error}"]
    attributes = getattr(metadata, "st_file_attributes", None)
    reparse = None if attributes is None else bool(attributes & 0x400)
    try:
        empty = next(parent.iterdir(), None) is None
    except OSError as error:
        return parent, [f"explicit test parent cannot be enumerated: {error}"]
    failures = precondition_failures(
        platform.system(),
        platform.machine(),
        64 if sys.maxsize > 2**32 else 32,
        True,
        parent.is_absolute(),
        stat.S_ISDIR(metadata.st_mode),
        reparse,
        empty,
    )
    if attributes is None:
        failures.append("Windows reparse-point attributes are unavailable")
    return parent, failures


def self_test():
    valid = precondition_failures("Windows", "AMD64", 64, True, True, True, False, True)
    assert not valid, valid
    assert precondition_failures("Linux", "x86_64", 64, False, False, False, False, False)
    passing_output = "\n".join(
        [*(f"test {name} ... ok" for name in EXPECTED_TESTS),
         f"test result: ok. {len(EXPECTED_TESTS)} passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 1.00s"]
    )
    assert not libtest_failures(passing_output, 0)
    assert libtest_failures("test result: ok. 0 passed; 0 failed; 0 ignored; 10 filtered out", 0)
    assert libtest_failures(passing_output.replace(" ... ok", " ... ignored"), 0)
    assert "taskkill" in windows_tree_kill_command(1234)
    assert tasklist_contains_pid('"cargo.exe","1234","Console","1","2,000 K"', 1234)
    assert not tasklist_contains_pid('"cargo.exe","1234","Console","1","2,000 K"', 4321)
    previous_parent = os.environ.get(PARENT_ENV)
    with tempfile.TemporaryDirectory(prefix="semaprax-windows-gate-self-test-") as directory:
        os.environ[PARENT_ENV] = directory
        try:
            _, parent_failures = inspect_parent()
        finally:
            if previous_parent is None:
                os.environ.pop(PARENT_ENV, None)
            else:
                os.environ[PARENT_ENV] = previous_parent
    assert "explicit test parent is not empty" not in parent_failures, parent_failures
    if platform.system() != "Windows":
        assert "host operating system is not Windows" in parent_failures, parent_failures
    print("self-test passed: gate rejects missing host/provisioning and zero/ignored runtime tests; no runtime evidence produced")
    return 0


def windows_tree_kill_command(pid):
    return ["taskkill", "/T", "/F", "/PID", str(pid)]


def tasklist_contains_pid(output, pid):
    return any(len(row) > 1 and row[1] == str(pid) for row in csv.reader(output.splitlines()))


def terminate_timed_out_process(process):
    """Kill Cargo and its Windows process tree; fail if quiescence is unclear."""
    if os.name != "nt":
        process.kill()
        process.wait(timeout=TERMINATION_TIMEOUT_SECONDS)
        return

    try:
        killed = subprocess.run(
            windows_tree_kill_command(process.pid),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=TERMINATION_TIMEOUT_SECONDS,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError(f"could not terminate timed-out Cargo process tree: {error}") from error
    if killed.returncode != 0:
        raise RuntimeError(
            "taskkill failed to terminate timed-out Cargo process tree "
            f"(status {killed.returncode}): {killed.stdout}"
        )
    try:
        process.communicate(timeout=TERMINATION_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        raise RuntimeError("timed-out Cargo process pipes did not settle after taskkill /T /F") from error
    if process.poll() is None:
        raise RuntimeError("timed-out Cargo process remained alive after taskkill /T /F")

    # Require taskkill's tree-termination report and independently verify the
    # direct Cargo PID is absent. Descendants reparented before verification
    # are not independently enumerated here.
    deadline = time.monotonic() + TERMINATION_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        try:
            probe = subprocess.run(
                ["tasklist", "/FI", f"PID eq {process.pid}", "/FO", "CSV", "/NH"],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=5,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise RuntimeError(f"could not verify Cargo PID termination: {error}") from error
        if probe.returncode != 0:
            raise RuntimeError(f"could not verify Cargo PID termination: {probe.stdout}")
        if not tasklist_contains_pid(probe.stdout, process.pid):
            return
        time.sleep(0.1)
    raise RuntimeError("could not prove the timed-out Cargo PID is absent")


def run_gate():
    parent, failures = inspect_parent()
    if failures:
        for failure in failures:
            print(f"Windows confinement gate refusal: {failure}", file=sys.stderr)
        return 2

    command = [
        "cargo",
        "test",
        "--locked",
        "--offline",
        "-p",
        PACKAGE,
        "--lib",
        FILTER,
        "--",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]
    print(f"provisioned Windows test parent: {parent}")
    print(f"running all {len(EXPECTED_TESTS)} named restricted-token, child-launch, job, ACL, and settlement tests")
    try:
        process = subprocess.Popen(
            command,
            cwd=REPO_ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
    except OSError as error:
        print(f"Windows confinement gate refusal: could not start Cargo: {error}", file=sys.stderr)
        return 1
    try:
        output, _ = process.communicate(timeout=RUNTIME_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        partial = error.stdout or ""
        if isinstance(partial, bytes):
            partial = partial.decode("utf-8", errors="replace")
        print(partial, end="")
        try:
            terminate_timed_out_process(process)
        except RuntimeError as termination_error:
            print(
                "Windows confinement gate refusal: runtime exceeded "
                f"{RUNTIME_TIMEOUT_SECONDS}s and process-tree quiescence is unproven: "
                f"{termination_error}",
                file=sys.stderr,
            )
            return 1
        print(
            f"Windows confinement gate refusal: runtime suite exceeded {RUNTIME_TIMEOUT_SECONDS} seconds; "
            "taskkill reported tree termination and the direct Cargo PID is absent",
            file=sys.stderr,
        )
        return 1
    print(output, end="")
    failures = libtest_failures(output, process.returncode)
    if failures:
        for failure in failures:
            print(f"Windows confinement gate refusal: {failure}", file=sys.stderr)
        return 1
    print(f"all {len(EXPECTED_TESTS)} explicitly selected Windows runtime tests executed and passed")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="exercise refusal and nonzero-result parsing only")
    parser.add_argument("--plan", action="store_true", help="print the exact runtime test selector and prerequisites")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.plan:
        print(f"host: Windows x86-64 or AArch64, 64-bit process")
        print(f"required provisioned parent: {PARENT_ENV} (existing, empty, non-reparse directory)")
        print(f"Cargo selector: -p {PACKAGE} --lib {FILTER} -- --ignored --nocapture --test-threads=1")
        for test in EXPECTED_TESTS:
            print(f"required executed test: {test}")
        print("selected tests use a deterministic test-only signing key; this is not release trust or request/bundle transport evidence")
        return 0
    return run_gate()


if __name__ == "__main__":
    raise SystemExit(main())
