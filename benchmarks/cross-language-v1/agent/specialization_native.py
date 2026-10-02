"""Darwin-only private scoring, using the existing frozen run.py oracle.

No native execution fallback exists on another host. Each phase sees its own
scratch subtree, the exact copied compiler, and protected system libraries.
The original 120-second process-group/output caps are reused unchanged.
"""
from __future__ import annotations
import base64
import importlib.util
import json
import os
import pathlib
import shutil
import signal
import sys
import tempfile
import time

import runnable_adapter as bounds
import runnable_v3_provenance as provenance
from .local_ollama import LocalTransportError, digest
from .specialization_inputs import CANDIDATES, PREFIX, public_tree

SANDBOX = pathlib.Path("/usr/bin/sandbox-exec")


def sandbox_policy(tool: pathlib.Path, phase: pathlib.Path) -> str:
    for path in (tool, phase):
        if not path.is_absolute() or path != path.resolve() or path.is_symlink():
            raise LocalTransportError("noncanonical_sandbox_authority")
    literal = lambda p: "(literal " + json.dumps(str(p)) + ")"
    subtree = lambda p: "(subpath " + json.dumps(str(p)) + ")"
    ancestors = sorted({str(x) for path in (tool, phase) for x in path.parents})
    # Rust's primary-thread stack guard uses anonymous fixed-address mapping.
    # On current Darwin, a deny-default profile turns that required VM primitive
    # into EINVAL before `main`. Start from the OS VM baseline, then explicitly
    # withdraw every file, process and network authority this scorer must not
    # have. The later narrow allows are the complete execution authority.
    return ("(version 1)(allow default)(deny network*)(deny process-fork)"
            "(deny process-exec)(allow process-exec " + literal(tool) + ")"
            "(deny file-read*)(allow file-read* " + literal(tool) + " " + subtree(phase) +
            ' (subpath "/usr/lib") (subpath "/System/Library"))'
            "(allow file-read-metadata " + " ".join(literal(x) for x in ancestors) + ")"
            "(deny file-write*)(allow file-write* " + subtree(phase) + ")"
            '(allow file-read* file-write* (literal "/dev/null"))'
            '(allow sysctl-read (sysctl-name "hw.memsize") (sysctl-name "hw.ncpu")'
            ' (sysctl-name "hw.activecpu") (sysctl-name "hw.logicalcpu")'
            ' (sysctl-name "kern.osrelease"))')


class NativeScorer:
    def __init__(self, sources, compiler: pathlib.Path, compiler_sha256: str):
        self.sources = sources
        self.original = pathlib.Path(compiler)
        self.expected = compiler_sha256
        self.temporary = None
        self.commands = []

    def __enter__(self):
        try:
            # Reuse, do not broaden, the already specified Darwin host profile.
            self.host = provenance.host_identity()
            self.sandbox_sha = digest(provenance.read_regular(SANDBOX, 1024 * 1024))
            data = provenance.read_regular(self.original, bounds.MAX_TOOL_BYTES)
            if digest(data) != self.expected or not os.access(self.original, os.X_OK):
                raise LocalTransportError("compiler_identity_or_mode_refused")
            self.temporary = tempfile.TemporaryDirectory(prefix="spx-specialization-")
            self.root = pathlib.Path(self.temporary.name).resolve()
            self.tool = self.root / "semaprax"
            bounds._write_snapshot_file(self.tool, data, 0o500)
            self.version_phase = self.root / "version"
            self.version_phase.mkdir(mode=0o700)
            self.phases = {self.version_phase}
            self.deadline = time.monotonic() + bounds.MAX_TIMEOUT_SECONDS
            path = provenance.ROOT / PREFIX / "run.py"
            spec = importlib.util.spec_from_file_location("specialization_frozen_scorer", path)
            self.scorer = importlib.util.module_from_spec(spec)
            sys.modules[spec.name] = self.scorer
            exec(compile(sources_bytes(self.sources, "run.py"), str(path), "exec"), self.scorer.__dict__)
            self.scorer.run_command = self.command
            self.scorer.scratch_dir = self.scratch
            document = json.loads(sources_bytes(self.sources, "adapters.json"))
            self.adapter = next(row for row in document["adapters"] if row["id"] == "semaprax-project")
            if (self.adapter.get("run_command") != ["{semaprax}", "run", "."] or
                    self.adapter.get("build_command") is not None or
                    self.adapter.get("success") != {"kind": "stdout_equals", "value": "0"}):
                raise LocalTransportError("unexpected_scoring_adapter")
            code, out, err = self.command([str(self.tool), "version"], self.version_phase)
            if code != 0 or not out.strip():
                raise LocalTransportError("sandboxed_compiler_preflight_failed:" + err[-200:])
            self.compiler_version = out.strip()
            return self
        except BaseException:
            self.close()
            raise

    def scratch(self, label):
        phase = pathlib.Path(tempfile.mkdtemp(prefix=label + "-", dir=self.root)).resolve()
        self.phases.add(phase)
        return phase

    def command(self, command, cwd, execution=None):
        if command == [str(self.tool), "version"]:
            cwd = self.version_phase
        elif command != [str(self.tool), "run", "."] or pathlib.Path(cwd) not in self.phases:
            raise LocalTransportError("unbound_compiler_command")
        if digest(provenance.read_regular(SANDBOX, 1024 * 1024)) != self.sandbox_sha:
            raise LocalTransportError("sandbox_identity_drift")
        if digest(provenance.read_regular(self.tool, bounds.MAX_TOOL_BYTES)) != self.expected:
            raise LocalTransportError("compiler_snapshot_drift")
        policy = sandbox_policy(self.tool, pathlib.Path(cwd))
        # The frozen process runner is bounded but has no KeyboardInterrupt
        # cleanup. Defer SIGINT until its group has been collected; do not
        # orphan a candidate process by catching an interrupt outside it.
        prior_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT})
        try:
            code, out, err, failure = bounds._run_bounded_group(
                [str(SANDBOX), "-p", policy, *command], pathlib.Path(cwd), self.deadline,
                dict(bounds.CLOSED_ENVIRONMENT))
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, prior_mask)
        row = {"argv": command, "cwd": str(cwd), "policy": policy,
               "exit_code": code, "failure": failure,
               "stdout": out.decode("utf-8", "replace"), "stderr": err.decode("utf-8", "replace")}
        # Base64 retains exact bounded output bytes without JSON control-byte
        # amplification; the owning scorer still receives decoded text.
        self.commands.append({"argv": command, "cwd": str(cwd), "policy": policy,
                              "exit_code": code, "failure": failure,
                              "stdout_base64": base64.b64encode(out).decode("ascii"),
                              "stderr_base64": base64.b64encode(err).decode("ascii")})
        return (None if failure else code), row["stdout"], failure or row["stderr"]

    def score(self, task_id, candidate_files):
        if set(candidate_files) != set(CANDIDATES[task_id]):
            raise LocalTransportError("candidate_path_inventory_mismatch")
        if any(not isinstance(x, str) for x in candidate_files.values()):
            raise LocalTransportError("candidate_content_not_text")
        self.deadline = time.monotonic() + bounds.MAX_TIMEOUT_SECONDS
        source = self.scratch("source")
        self.commands = []
        before = 0
        try:
            public = public_tree(self.sources, task_id)
            hidden_prefix = PREFIX + "tasks/" + task_id + "/hidden/semaprax/"
            hidden = {name[len(hidden_prefix):]: data for name, data in self.sources.items()
                      if name.startswith(hidden_prefix)}
            # Never let an overlay replace a model-authored candidate with a
            # reference implementation and then falsely accept the model.
            if not hidden or set(hidden).intersection(candidate_files):
                raise LocalTransportError("hidden_overlay_replaces_candidate")
            for phase, inventory in (("public", public), ("hidden", hidden)):
                for name, data in inventory.items():
                    if phase == "public" and name in candidate_files:
                        data = candidate_files[name].encode("utf-8")
                    bounds._write_snapshot_file(source / phase / name, data)
            task = {"id": task_id, "category": "specialization",
                    "languages": {"semaprax-project": {"public": "public", "hidden": "hidden"}}}
            result = self.scorer.evaluate_pair(source, task, "semaprax-project", self.adapter, str(self.tool))
            return {"result": result, "commands": self.commands[before:]}
        finally:
            shutil.rmtree(source, ignore_errors=True)
            # evaluate_pair removes its scratch phases; forget their authority.
            self.phases = {p for p in self.phases if p.is_dir()}

    def close(self):
        if self.temporary is not None:
            self.temporary.cleanup()
            self.temporary = None

    def __exit__(self, *_):
        self.close()


def sources_bytes(sources, relative):
    return sources[PREFIX + relative]
