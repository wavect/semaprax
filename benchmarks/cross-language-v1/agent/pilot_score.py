"""Additive candidate session; the fixed-source v3 public route is unchanged."""
from __future__ import annotations
import base64
import json
import pathlib
import shutil
import time
import uuid
from . import pilot_protocol as p
import runnable_adapter_v3 as v3


COMPLETION = "SEMAPRAX_PILOT_COMPLETED_V1"


def numeric_bridge(compiled):
    """Assertion isolation only; the OS authority sandbox remains mandatory."""
    return ("'use strict';\nconst vm = require('node:vm');\n"
            "const context = vm.createContext(Object.create(null), {codeGeneration:{strings:false,wasm:false},microtaskMode:'afterEvaluate'});\n"
            "vm.runInContext('globalThis.exports = Object.create(null);', context, {timeout:100});\n"
            "vm.runInContext(" + json.dumps(compiled, ensure_ascii=True) + ", context, {timeout:100});\n"
            "exports.validate = function(kind,version,payloadLen) {\n"
            "  const args = [kind,version,payloadLen];\n"
            "  if (!args.every(x => typeof x === 'number' && Number.isSafeInteger(x))) throw new Error('nonprimitive argument');\n"
            "  const result = vm.runInContext('exports.validate(' + args.map(x=>JSON.stringify(x)).join(',') + ')', context, {timeout:100});\n"
            "  if (typeof result !== 'number' || !Number.isSafeInteger(result)) throw new Error('nonprimitive result');\n"
            "  return result;\n};\n").encode()


def install_assertion_bridge(directory):
    directory = pathlib.Path(directory)
    compiled = p.provenance.read_regular(directory / 'validate.js', 65536)
    harness = p.provenance.read_regular(directory / 'index.js', 65536)
    bridge = numeric_bridge(compiled.decode())
    (directory / 'validate.js').write_bytes(bridge)
    completed = harness + ("\n;process.stdout.write(" + json.dumps("\n" + COMPLETION + "\n") + ");\n").encode()
    (directory / 'index.js').write_bytes(completed)
    return compiled, bridge, completed


class CandidateSession(v3.OfficialSession):
    def _command(self, command, cwd, execution=None):
        if len(command) == 2 and command[1] == "index.js":
            compiled, bridge, harness = install_assertion_bridge(cwd)
            for label, data in (("candidate-compiled.js", compiled), ("candidate-bridge.js", bridge), ("harness-completion.js", harness)):
                self.artifacts.append({"path": "pilot/" + pathlib.Path(cwd).parent.name + "/" + pathlib.Path(cwd).name + "/" + label,
                                       "bytes": len(data), "sha256": p.digest(data), "base64": base64.b64encode(data).decode()})
        return super()._command(command, cwd, execution)

    def _stage(self, directory):
        result = super()._stage(directory)
        if result["passed"]:
            observed = self.authority.commands[-1]["stdout"]
            if observed != "ok\n\n" + COMPLETION + "\n":
                return {"passed": False, "phase": "run", "detail": ["host_harness_completion_missing"]}
        return result

    def admit_pilot(self, plan):
        # __enter__ already checked independent archive receipts, exact Darwin
        # host, runtime loader and physical authority probes before any model.
        manifest, sources, task, public, hidden, _ = p.source_inputs()
        if manifest != self.manifest or sources != self.sources:
            raise ValueError("pilot_source_drifted")
        p.admit_paths(public, hidden, plan["candidate_paths"])
        if plan["profile"] != p.PROFILE or plan["task_id"] != p.TASK:
            raise ValueError("pilot_profile_not_admitted")
        return task, public, hidden

    def score_candidate(self, plan, candidate):
        task, public_bytes, hidden_bytes = self.admit_pilot(plan)
        p.exact(candidate, (p.CANDIDATE,), "candidate_file_set_refused")
        source = candidate[p.CANDIDATE]
        if not isinstance(source, str) or not source or "\x00" in source or len(source.encode()) > plan["configuration"]["limits"]["max_result_bytes"]:
            raise ValueError("candidate_text_refused")
        source = source.encode()
        parent = self.root / ("pilot-" + uuid.uuid4().hex)
        parent.mkdir(mode=0o700)
        self.deadline = time.monotonic() + v3.v1.MAX_TIMEOUT_SECONDS
        start = len(self.authority.commands)
        record = {"task_id": p.TASK, "adapter_id": "typescript", "classification": "live_pilot_candidate",
                  "candidate_sha256": p.digest(source), "source_manifest_sha256": p.provenance.SOURCE_HASH}
        try:
            for phase, overlay in (("public", {}), ("hidden", hidden_bytes)):
                directory = parent / phase
                directory.mkdir(mode=0o700)
                files = {**public_bytes, **overlay, p.CANDIDATE: source}
                # Disjointness is an admission requirement, not overwrite order.
                p.admit_paths(public_bytes, hidden_bytes, (p.CANDIDATE,))
                for name, data in files.items():
                    v3.v1._write_snapshot_file(directory / name, data)
                if p.provenance.read_regular(directory / p.CANDIDATE, 65536) != source:
                    raise ValueError("phase_candidate_binding_refused")
                record[phase] = self._stage(directory)
                self._capture(directory, "pilot/" + parent.name + "/" + phase)
                # Retain both phase results, including unsuccessful candidates.
            record["status"] = "ok" if record["public"]["passed"] and record["hidden"]["passed"] else "failed"
            record["candidate_in_both_phases"] = True
            self.artifacts.append({"path": "pilot/" + parent.name + "/validate.ts", "bytes": len(source),
                                   "sha256": p.digest(source), "base64": base64.b64encode(source).decode()})
            self.results.append(record)
            return record
        finally:
            record["command_indices"] = list(range(start, len(self.authority.commands)))
            shutil.rmtree(parent)

    def evidence(self):
        bundle = super().evidence()
        bundle["result"].update(schema="benchmark.cross_language.live_pilot_scoring.v1",
                                 status="candidate_execution_observed", profile=p.PROFILE)
        return bundle
