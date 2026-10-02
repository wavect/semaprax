"""Explicitly pinned Apple Container Linux scoring; no provider dispatch authority."""
from __future__ import annotations
import base64
import hashlib
import importlib.util
import io
import json
import lzma
import os
import pathlib
import re
import sys
import tarfile
import tempfile
import time
import uuid
from . import pilot_protocol as p
from .pilot_score import CandidateSession
import runnable_adapter_v3 as v3
import runnable_v3_extraction as extraction

PROFILE = "apple-container-linux-arm64-typescript-pilot.v1"
SCHEMA = "benchmark.cross_language.linux_host_provision.v1"
NODE_NAME = "node-v22.12.0-linux-arm64.tar.xz"
NODE_ROOT = "node-v22.12.0-linux-arm64"
LAUNCHER_SOURCE = pathlib.Path(__file__).with_name("pilot_linux_launcher.c")
HASH = re.compile(r"[0-9a-f]{64}")
EXPECTED_PROBE = {"phase_read_write": True, "sibling_read": "EACCES", "sibling_write": "EACCES",
                  "fork": "EPERM", "socket": "EPERM"}


def admit_provision(data, expected_digest):
    """Caller independently reviews/pins this receipt; self-hashing is insufficient."""
    if not isinstance(expected_digest, str) or not HASH.fullmatch(expected_digest) or p.digest(data) != expected_digest:
        raise ValueError("linux_provision_digest_refused")
    value = p.strict_json(data)
    if p.canonical(value) != data:
        raise ValueError("linux_provision_not_canonical")
    p.exact(value, ("schema", "profile", "container_path", "container_sha256", "image", "arm64_manifest_sha256",
                    "kernel_release", "node_binary_sha256", "launcher_sha256", "launcher_source_sha256",
                    "review_reference"), "linux_provision_shape_refused")
    if value["schema"] != SCHEMA or value["profile"] != PROFILE:
        raise ValueError("linux_profile_refused")
    for field in ("container_sha256", "arm64_manifest_sha256", "node_binary_sha256", "launcher_sha256", "launcher_source_sha256"):
        if not isinstance(value[field], str) or not HASH.fullmatch(value[field]):
            raise ValueError("linux_pin_refused")
    if not isinstance(value["image"], str) or not re.fullmatch(r"docker\.io/library/rust@sha256:[0-9a-f]{64}", value["image"]):
        raise ValueError("linux_image_must_be_immutable")
    if (not isinstance(value["kernel_release"], str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", value["kernel_release"])
            or not isinstance(value["review_reference"], str) or not 1 <= len(value["review_reference"]) <= 1024):
        raise ValueError("linux_review_or_kernel_refused")
    path = pathlib.Path(value["container_path"])
    if not path.is_absolute() or path != path.resolve() or any(x in str(path) for x in (",", "\n", "\x00")):
        raise ValueError("linux_container_path_refused")
    if value["launcher_source_sha256"] != p.digest(p.provenance.read_regular(LAUNCHER_SOURCE, 65536)):
        raise ValueError("linux_launcher_source_drifted")
    return value


def linux_node(data, expected_binary_digest):
    """Bounded selected-member extraction after the official checksum admission."""
    decoded = extraction.LimitedDecoded(lzma.LZMAFile(io.BytesIO(data)), 512 * 1024 * 1024)
    result = None
    names = set()
    try:
        with tarfile.open(fileobj=decoded, mode="r|", tarinfo=extraction.BoundedInfo) as archive:
            for index, member in enumerate(archive):
                name = member.name
                parts = name.split("/")
                if (index >= 10000 or len(name.encode()) > 4096 or name in names or
                        parts[0] != NODE_ROOT or any(x in ("", ".", "..") for x in parts) or
                        not (member.isreg() or member.isdir() or member.issym())):
                    raise ValueError("linux_node_archive_member_refused")
                names.add(name)
                if name == NODE_ROOT + "/bin/node":
                    if not member.isreg() or member.size > 128 * 1024 * 1024:
                        raise ValueError("linux_node_binary_type_refused")
                    with archive.extractfile(member) as stream:
                        result = stream.read(member.size + 1)
                    if len(result) != member.size:
                        raise ValueError("linux_node_binary_truncated")
        while decoded.read(65536):
            pass
    finally:
        decoded.stream.close()
    # ELF64 little-endian, AArch64, ET_EXEC or PIE. Loader comes from pinned image.
    if (result is None or p.digest(result) != expected_binary_digest or len(result) < 64
            or result[:6] != b"\x7fELF\x02\x01" or result[18:20] != b"\xb7\x00"
            or result[16:18] not in (b"\x02\x00", b"\x03\x00")):
        raise ValueError("linux_node_binary_identity_refused")
    return result


def prepare(directory, runtime_root, pins):
    receipt = p.provenance.read_regular(directory / "node22.12.0-SHASUMS256.txt", 65536)
    metadata = p.provenance.read_regular(directory / "typescript5.8.3-registry.json", 65536)
    if p.digest(receipt) != p.provenance.NODE_RECEIPT_HASH or p.digest(metadata) != p.provenance.TS_RECEIPT_HASH:
        raise ValueError("linux_official_receipt_drifted")
    lines = [line.split() for line in receipt.decode("ascii").splitlines() if line.endswith("  " + NODE_NAME)]
    if len(lines) != 1 or len(lines[0]) != 2 or not HASH.fullmatch(lines[0][0]):
        raise ValueError("linux_official_node_checksum_missing")
    archive = p.provenance.read_regular(directory / NODE_NAME, 64 * 1024 * 1024)
    typescript = p.provenance.read_regular(directory / p.provenance.TS_NAME, 16 * 1024 * 1024)
    if p.digest(archive) != lines[0][0] or base64.b64encode(hashlib.sha512(typescript).digest()).decode() != p.provenance.TS_INTEGRITY:
        raise ValueError("linux_official_archive_drifted")
    launcher = p.provenance.read_regular(directory / "pilot-linux-launcher", 1024 * 1024)
    if p.digest(launcher) != pins["launcher_sha256"]:
        raise ValueError("linux_launcher_binary_drifted")
    node = linux_node(archive, pins["node_binary_sha256"])
    contents = {"node": node, "launcher": launcher}
    contents.update(extraction.scan_archive(typescript, "typescript"))
    runtime = extraction.Runtime(runtime_root, contents)
    os.chmod(runtime.root, 0o700)
    os.chmod(runtime.root / "launcher", 0o500)
    next(row for row in runtime.inventory if row["path"] == "launcher")["mode"] = 0o500
    os.chmod(runtime.root, 0o500)
    runtime.check()
    return runtime, {"node_archive_sha256": p.digest(archive), "typescript_archive_sha256": p.digest(typescript),
                     "runtime_inventory": runtime.inventory, "node_checksum_receipt_sha256": p.digest(receipt),
                     "typescript_registry_receipt_sha256": p.digest(metadata)}


class LinuxAuthority:
    def __init__(self, runtime, pins):
        self.runtime, self.pins, self.commands = runtime, pins, []
        self.cli = pathlib.Path(pins["container_path"])
        # Host control plane only. These values are never inherited by the guest.
        self.environment = {"HOME": str(pathlib.Path.home()), "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}
        self.check()

    def check(self):
        self.runtime.check()
        if p.provenance.file_digest(self.cli, 128 * 1024 * 1024)[1] != self.pins["container_sha256"]:
            raise ValueError("linux_container_executable_drifted")

    def control(self, arguments, seconds=15):
        return v3.v1._run_bounded_group([str(self.cli), *arguments], self.runtime.root.parent,
                                        time.monotonic() + seconds, self.environment)

    def admit_image(self):
        code, out, err, reason = self.control(["image", "inspect", self.pins["image"]])
        if reason or code != 0:
            raise ValueError("linux_pinned_image_not_cached")
        rows = json.loads(out)
        if len(rows) != 1 or rows[0]["configuration"]["descriptor"]["digest"] != self.pins["image"].split("@")[1]:
            raise ValueError("linux_image_identity_refused")
        variants = [v for v in rows[0]["variants"] if v["platform"].get("architecture") == "arm64" and v["platform"].get("os") == "linux"]
        if len(variants) != 1 or variants[0]["digest"] != "sha256:" + self.pins["arm64_manifest_sha256"]:
            raise ValueError("linux_image_platform_refused")
        return {"image": self.pins["image"], "arm64_manifest": variants[0]["digest"]}

    def argv(self, command, cwd, name, denied=None):
        cwd = pathlib.Path(cwd)
        if cwd != cwd.resolve() or not cwd.is_dir() or any(x in str(cwd) for x in (",", "\n")):
            raise ValueError("linux_phase_path_refused")
        root = str(self.runtime.root)
        if any(x in root for x in (",", "\n")):
            raise ValueError("linux_runtime_path_refused")
        if command[0] not in (str(self.runtime.node), "--probe"):
            raise ValueError("linux_unbound_command_refused")
        translated = ["/runtime/" + x[len(root) + 1:] if x.startswith(root + "/") else x for x in command]
        return ["run", "--name", name, "--progress", "none", "--platform", "linux/arm64",
                "--network", "none", "--no-dns", "--read-only", "--cap-drop", "ALL",
                "--uid", str(os.getuid()), "--gid", str(os.getgid()), "--cpus", "1", "--memory", "512M",
                "--ulimit", "nproc=64:64", "--ulimit", "nofile=128:128", "--ulimit", "fsize=8388608:8388608",
                "--workdir", "/phase", "--mount", "type=bind,source=" + root + ",target=/runtime,readonly",
                "--mount", "type=bind,source=" + str(cwd) + ",target=/phase",
                *(["--mount", "type=bind,source=" + str(denied) + ",target=/denied"] if denied else []),
                "--entrypoint", "/usr/bin/env", self.pins["image"], "-i", "HOME=/unavailable", "PATH=/unavailable",
                "LANG=C", "LC_ALL=C", "OPENSSL_CONF=/dev/null", "UV_THREADPOOL_SIZE=2", "/runtime/launcher", *translated]

    def launch(self, command, cwd, deadline, *, denied=None):
        self.check()
        name = "spx-pilot-" + uuid.uuid4().hex
        arguments = self.argv(command, cwd, name, denied)
        try:
            code, out, err, reason = v3.v1._run_bounded_group([str(self.cli), *arguments], pathlib.Path(cwd), deadline, self.environment)
        finally:
            # Kill/delete by exact private name even if the attached CLI timed out.
            cleanup, _, _, failed = self.control(["delete", "--force", name])
            if cleanup != 0 or failed:
                raise ValueError("linux_container_cleanup_unconfirmed")
        self.commands.append({"argv": command, "container_argv": arguments, "cwd": str(cwd), "status": code,
                              "failure": reason, "stdout": out.decode("utf-8", "replace"),
                              "stdout_base64": base64.b64encode(out).decode(), "stderr_base64": base64.b64encode(err).decode()})
        return (None if reason else code), out.decode("utf-8", "replace"), reason or err.decode("utf-8", "replace")

    def preflight(self, root):
        if os.getuid() == 0 or os.getgid() == 0:
            raise ValueError("linux_nonroot_required")
        phase, denied = root / "authority-phase", root / "authority-denied"
        phase.mkdir(mode=0o700)
        denied.mkdir(mode=0o700)
        canary = denied / "canary"
        canary.write_bytes(b"private-positive-control")
        # Read/write both actually succeed before the same file is restricted.
        with canary.open("r+b") as stream:
            if stream.read() != b"private-positive-control":
                raise ValueError("linux_canary_control_failed")
            stream.seek(0)
            stream.write(b"private-positive-control")
        code, out, err = self.launch(["--probe"], phase, time.monotonic() + 30, denied=denied)
        if code != 0 or json.loads(out) != EXPECTED_PROBE or canary.read_bytes() != b"private-positive-control":
            raise ValueError("linux_authority_probe_failed:" + err[-200:])
        return {"physical_denials": EXPECTED_PROBE, "host_canary_positive_read_write": True,
                "guest_positive_controls_before_restriction": ["canary_open_read_write", "fork_wait", "socket_create"],
                "outer_network": "none", "process_authority": "seccomp denies non-thread clone; clone3 ENOSYS",
                "filesystem_authority": "Landlock ABI >=3; only runtime, guest loader libraries and phase",
                "source_separation": "only current phase mounted; sibling canary denied under same policy"}


class LinuxCandidateSession(CandidateSession):
    """Scoring only. Does not authorize or impersonate a Linux model transport."""
    def __init__(self, provenance_directory, *, expected_provision_sha256):
        super().__init__(provenance_directory)
        self.provision_digest = expected_provision_sha256

    def __enter__(self):
        try:
            self.host = p.provenance.host_identity()  # Apple control-plane admission, not guest identity.
            data = p.provenance.read_regular(self.provenance_directory / "linux-host.json", 65536)
            self.pins = admit_provision(data, self.provision_digest)
            self.manifest, baseline = p.provenance.source_snapshot()
            self.correction = v3.corrections.admit(self.manifest, baseline)
            self.sources = self.correction["sources"]
            self.subject = v3.execution_subject()
            self.temporary = tempfile.TemporaryDirectory(prefix="pilot-linux-", dir=self.provenance_directory)
            self.root = pathlib.Path(self.temporary.name).resolve()
            runtime_root = self.root / "runtime"
            runtime_root.mkdir(mode=0o700)
            self.runtime, self.provenance = prepare(self.provenance_directory, runtime_root, self.pins)
            self.authority = LinuxAuthority(self.runtime, self.pins)
            image = self.authority.admit_image()
            self.observations.append(self.authority.preflight(self.root))
            self.deadline = time.monotonic() + 60
            identity = "JSON.stringify({os:process.platform,arch:process.arch,kernel:require('os').release()})"
            code, out, _ = self._command([str(self.runtime.node), "-p", identity], self.root / "authority-phase")
            if code != 0 or json.loads(out) != {"os": "linux", "arch": "arm64", "kernel": self.pins["kernel_release"]}:
                raise ValueError("linux_guest_identity_refused")
            self.host = {"control_plane": self.host, "execution": json.loads(out), **image,
                         "classification": "local_Apple_Container_VM_not_independent_physical_hardware"}
            spec = importlib.util.spec_from_file_location("semaprax_linux_pilot_scorer", v3.SUITE / "run.py")
            self.scorer = importlib.util.module_from_spec(spec)
            sys.modules[spec.name] = self.scorer
            exec(compile(self.sources["benchmarks/cross-language-v1/run.py"], str(v3.SUITE / "run.py"), "exec"), self.scorer.__dict__)
            self.scorer.run_command = self._command
            bound = {"node": self.runtime.node, "typescript_lib": self.runtime.compiler}
            self.adapter = next(row for row in json.loads(v3.v2._snapshot_adapter(self.sources["benchmarks/cross-language-v1/adapters.json"], "typescript", bound))["adapters"] if row["id"] == "typescript")
            for command, expected in (([str(self.runtime.node), "--version"], "v22.12.0"),
                                      ([str(self.runtime.node), str(self.runtime.compiler), "--version"], "Version 5.8.3")):
                code, out, _ = self._command(command, self.root / "authority-phase")
                if code != 0 or out.strip() != expected:
                    raise ValueError("linux_toolchain_version_refused")
            return self
        except Exception as error:
            error.official_observations = list(self.observations)
            error.official_commands = list(self.authority.commands) if self.authority else []
            self.close()
            raise

    def admit_pilot(self, plan):
        manifest, sources, task, public, hidden, _ = p.source_inputs()
        if manifest != self.manifest or sources != self.sources:
            raise ValueError("linux_pilot_source_drifted")
        p.admit_paths(public, hidden, plan["candidate_paths"])
        # Future shared plans must bind this exact independently reviewed receipt.
        if (plan.get("profile") != "typescript-live-pilot.v1"
                or plan.get("execution_profiles", {}).get("linux-arm64") != {"profile": PROFILE, "provision_sha256": self.provision_digest}
                or plan.get("task_id") != p.TASK):
            raise ValueError("linux_pilot_profile_not_admitted")
        return task, public, hidden

    def evidence(self):
        result = {"schema": "benchmark.cross_language.linux_pilot_scoring.v1", "profile": PROFILE,
                  "status": "candidate_execution_observed", "host": self.host, "execution_subject": self.subject,
                  "provision_sha256": self.provision_digest, "pins": self.pins, "provenance": self.provenance,
                  "authority": self.observations, "commands": self.authority.commands, "results": self.results,
                  "source_manifest_sha256": p.provenance.SOURCE_HASH, "source_correction_sha256": v3.corrections.HASH,
                  "model_generation": "not_performed_or_authorized_by_scoring_session"}
        bundle = {"result": result, "source_manifest": self.manifest, "artifacts": self.artifacts + self.correction["artifacts"]}
        if len(p.canonical(bundle)) > v3.MAX_EVIDENCE_BYTES:
            raise ValueError("linux_evidence_capacity_exceeded")
        return bundle
