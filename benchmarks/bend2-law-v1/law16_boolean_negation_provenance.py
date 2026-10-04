#!/usr/bin/env python3
"""Capture bounded, machine-local provenance for LAW-16 Boolean process evidence.

The collector records only commands and files observed during this invocation.
It deliberately records fresh/repeat child-process separation independently from
cache state: this runner has no authority to clear or verify host caches.
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys


SCHEMA = "semaprax.bend2-law-benchmark.boolean-negation-process-provenance.v1"
PROBE_TIMEOUT_SECONDS = 15


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def file_reference(path):
    data = path.read_bytes()
    return {"path": str(path.resolve()), "bytes": len(data), "sha256": digest(data)}


def observe(argv):
    """Return a bounded, replayable record of one local version or host probe."""
    try:
        completed = subprocess.run(argv, capture_output=True, timeout=PROBE_TIMEOUT_SECONDS)
        return {
            "argv": [str(part) for part in argv],
            "exit_code": completed.returncode,
            "stdout_utf8": completed.stdout.decode("utf-8", errors="replace"),
            "stderr_utf8": completed.stderr.decode("utf-8", errors="replace"),
        }
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"argv": [str(part) for part in argv], "unavailable": str(error)}


def host_observation():
    """Collect the platform's own host/OS facts without a hostname or cache claim."""
    system = os.uname().sysname
    host = {
        "system": system,
        "machine": os.uname().machine,
        "release": os.uname().release,
        "probes": {"uname": observe(["/usr/bin/uname", "-smr"])},
    }
    if system == "Darwin":
        host["operating_system"] = {"sw_vers": observe(["/usr/bin/sw_vers"])}
        host["hardware"] = {
            "cpu_brand": observe(["/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string"]),
            "physical_cpu_count": observe(["/usr/sbin/sysctl", "-n", "hw.physicalcpu"]),
            "logical_cpu_count": observe(["/usr/sbin/sysctl", "-n", "hw.logicalcpu"]),
            "memory_bytes": observe(["/usr/sbin/sysctl", "-n", "hw.memsize"]),
        }
    elif system == "Linux":
        host["operating_system"] = {"os_release": observe(["/usr/bin/env", "cat", "/etc/os-release"])}
        host["hardware"] = {
            "cpu": observe(["/usr/bin/env", "sh", "-c", "LC_ALL=C lscpu"]),
            "memory": observe(["/usr/bin/env", "sh", "-c", "LC_ALL=C head -n 3 /proc/meminfo"]),
        }
    else:
        host["operating_system"] = {"status": "observed_only_through_uname"}
        host["hardware"] = {"status": "unavailable_no_platform_probe"}
    return host


def command_sequence(receipts):
    sequence = []
    for receipt in receipts:
        for state in ("fresh_process", "repeat_process"):
            for ordinal, sample in enumerate(receipt["cells"][state]["samples"], start=1):
                sequence.append(
                    {
                        "lane": receipt["lane"],
                        "kind": receipt["kind"],
                        "state": state,
                        "ordinal": ordinal,
                        "argv": sample["argv"],
                        "command_sha256": sample["command_sha256"],
                    }
                )
    return sequence


def capture(*, bend_root, bun, semaprax, z3, bend_main, receipts):
    """Return provenance bound to the just-captured receipt command sequence."""
    tools = {
        "bun": {"identity": file_reference(bun), "version": observe([str(bun), "--version"])},
        "bend_main": {"identity": file_reference(bend_main)},
        "semaprax": {"identity": file_reference(semaprax), "version": observe([str(semaprax), "--version"])},
        "z3": {"identity": file_reference(z3), "version": observe([str(z3), "--version"])},
        "bend_repository": {
            "path": str((bend_root / "bend2").resolve()),
            "commit": observe(["/usr/bin/git", "-C", str(bend_root / "bend2"), "rev-parse", "HEAD"]),
        },
    }
    sequence = command_sequence(receipts)
    return {
        "schema": SCHEMA,
        "status": "observed_local_process_provisioning",
        "collector": {
            "script": file_reference(pathlib.Path(__file__)),
            "python": {"executable": str(pathlib.Path(sys.executable).resolve()), "version": sys.version},
        },
        "host": host_observation(),
        "toolchain": tools,
        "backend_and_flags": {
            "bend_verdict": {"backend": "Bend --verdict", "environment": {"BEND_NO_TELEMETRY": "1"}},
            "semaprax_z3": {
                "backend": "SEMAPRAX project-proof-check with installed Z3",
                "arguments": ["--tool", "z3", "--host-profile", "trusted-local", "--declaration", "app.negate", "--ensures", "0"],
            },
        },
        "process_states": {
            "fresh_process": "a new child process using the first byte-identical copied input or project path",
            "repeat_process": "a new child process using the second byte-identical copied input or project path",
        },
        "cold_cache": {
            "status": "unavailable",
            "reason": "the capture does not clear or verify OS page, executable, solver, or tool caches",
        },
        "command_sequence": sequence,
        "command_count": len(sequence),
        "nonclaims": [
            "fresh_process and repeat_process are not true cold-cache states",
            "host observation binds only this capture, not historical capsules",
            "no cross-route timing ratio or winner",
        ],
    }
