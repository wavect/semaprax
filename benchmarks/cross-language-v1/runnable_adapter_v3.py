#!/usr/bin/env python3
"""Official, fixed-source TypeScript conformance; no arbitrary candidate route."""
from __future__ import annotations
import argparse
import base64
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

SUITE = pathlib.Path(__file__).resolve().parent
if str(SUITE) not in sys.path:
    sys.path.insert(0, str(SUITE))
import runnable_adapter as v1
import runnable_adapter_v2 as v2
import runnable_v3_provenance as p
import runnable_v3_extraction as extraction
import runnable_v3_authority as authority

SCHEMA = "benchmark.cross_language.runnable_adapter.v3"
MUTANT_PATH = SUITE / "provenance/typescript-official-v3-mutants.json"
MUTANT_HASH = "a3dcb21947a3d87d799b9f80f1baf34c22ae2fe33076d88a9f88b8dfabc65ff3"
MAX_EVIDENCE_BYTES = 8 * 1024 * 1024


def unavailable(reason, observations=()):
    return {"schema": SCHEMA, "status": "unavailable", "reason": reason, "observations": list(observations)}


def execution_subject():
    git = pathlib.Path("/usr/bin/git")
    data = p.read_regular(git, 1024 * 1024)
    v1._admit_host_executable(git, "sha256:" + p.digest(data), "git")
    def run(args):
        result = subprocess.run([str(git), "-C", str(p.ROOT), *args], capture_output=True,
                                env=dict(v1.CLOSED_ENVIRONMENT), timeout=5, check=False)
        if result.returncode or len(result.stdout) > 65536:
            raise p.Error("execution_subject_unavailable")
        return result.stdout.decode().strip()
    files = []
    for file in sorted(SUITE.glob("runnable*v3*.py")):
        content = p.read_regular(file, 1024 * 1024)
        files.append({"path": file.name, "sha256": p.digest(content)})
    return {"git_head": run(["rev-parse", "HEAD"]), "worktree_dirty": bool(run(["status", "--porcelain"])),
            "implementation_files": files}


class OfficialSession:
    def __init__(self, provenance_directory):
        self.provenance_directory = pathlib.Path(provenance_directory)
        if (not self.provenance_directory.is_absolute() or len(str(self.provenance_directory).encode()) > 4096
                or self.provenance_directory != self.provenance_directory.resolve()
                or not self.provenance_directory.is_dir()):
            raise p.Error("provisioning_directory_authority_refused")
        self.temporary = None
        self.runtime = None
        self.authority = None
        self.artifacts = []
        self.results = []
        self.observations = []

    def __enter__(self):
        try:
            self.host = p.host_identity()
            self.manifest, self.sources = p.source_snapshot()
            self.subject = execution_subject()
            mutant_bytes = p.read_regular(MUTANT_PATH, 65536)
            if p.digest(mutant_bytes) != MUTANT_HASH:
                raise p.Error("approved_mutant_inventory_drifted")
            self.mutants = {row["task_id"]: row for row in json.loads(mutant_bytes)["mutants"]}
            self.temporary = tempfile.TemporaryDirectory(prefix="r15-v3-", dir=self.provenance_directory)
            self.root = pathlib.Path(self.temporary.name).resolve()
            runtime_root = self.root / "runtime"
            runtime_root.mkdir(mode=0o700)
            self.runtime, self.provenance = extraction.prepare(self.provenance_directory, runtime_root)
            # Frozen scorer helpers inspect only this bounded host-only tree,
            # reconstructed from admitted bytes. Node receives no read grant.
            self.host_sources = self.root / "host-source-snapshot"
            self.host_sources.mkdir(mode=0o700)
            for name, content in self.sources.items():
                v1._write_snapshot_file(self.host_sources / name, content)
            self.authority = authority.Authority(self.runtime)
            self.observations.append(self.authority.preflight(self.root))
            # Load the pinned existing scorer only after exact source admission.
            module_name = "semaprax_official_ts_existing_scorer"
            spec = importlib.util.spec_from_file_location(module_name, p.ROOT / "benchmarks/cross-language-v1/run.py")
            self.scorer = importlib.util.module_from_spec(spec)
            sys.modules[module_name] = self.scorer
            exec(compile(self.sources["benchmarks/cross-language-v1/run.py"], str(p.ROOT / "benchmarks/cross-language-v1/run.py"), "exec"), self.scorer.__dict__)
            self.scorer.run_command = self._command
            self.tasks = json.loads(self.sources["benchmarks/cross-language-v1/tasks.json"])["tasks"]
            adapters = self.sources["benchmarks/cross-language-v1/adapters.json"]
            bound = {"node": self.runtime.node, "typescript_lib": self.runtime.compiler}
            self.adapter = next(row for row in json.loads(v2._snapshot_adapter(adapters, "typescript", bound))["adapters"] if row["id"] == "typescript")
            self.deadline = time.monotonic() + v1.MAX_TIMEOUT_SECONDS
            for command, expected in (([str(self.runtime.node), "--version"], "v22.12.0"),
                                      ([str(self.runtime.node), str(self.runtime.compiler), "--version"], "Version 5.8.3")):
                code, out, err = self._command(command, self.root / "authority-phase")
                if code != 0 or out.strip() != expected:
                    raise p.Error("official_tool_version_disagrees:" + err)
            return self
        except Exception as error:
            error.official_observations = list(self.observations)
            error.official_commands = list(self.authority.commands) if self.authority else []
            self.close()
            raise

    def close(self):
        if self.temporary:
            if self.runtime:
                self.runtime.dispose()
            else:
                for directory, _, _ in os.walk(self.temporary.name):
                    os.chmod(directory, 0o700)
            self.temporary.cleanup()
            self.temporary = None

    def __exit__(self, *_):
        self.close()

    def _command(self, command, cwd, execution=None):
        return self.authority.launch(command, cwd, self.deadline)

    def _stage(self, directory):
        before = len(self.authority.commands)
        result = self.scorer.stage(directory, self.adapter, "")
        count = len(self.authority.commands) - before
        if (not isinstance(result, dict) or set(result) != {"passed", "phase", "detail"}
                or type(result["passed"]) is not bool or result["phase"] not in ("build", "run")
                or not isinstance(result["detail"], list) or not result["detail"]
                or not all(isinstance(value, str) for value in result["detail"])
                or (result["phase"] == "run" and count != 2)
                or (result["phase"] == "build" and (count != 1 or result["passed"]))):
            raise p.Error("scoring_result_shape_or_dispatch_refused")
        return result

    def _copy_phase(self, directory, prefix):
        directory.mkdir(mode=0o700)
        prefix += "/"
        for name, data in self.sources.items():
            if name.startswith(prefix):
                v1._write_snapshot_file(directory / name[len(prefix):], data)

    def _mutate(self, directory, mutant):
        path = directory / mutant["path"]
        content = p.read_regular(path, v1.MAX_SOURCE_FILE_BYTES).decode()
        if content.count(mutant["target"]) != 1:
            raise p.Error("mutant_target_missing_or_ambiguous")
        changed = content.replace(mutant["target"], mutant["replacement"])
        path.write_text(changed)
        return {"path": mutant["path"], "before_sha256": p.digest(content.encode()),
                "after_sha256": p.digest(changed.encode()), "target_count": 1}

    def _capture(self, directory, label):
        count = 0
        for parent, dirs, files in os.walk(directory, followlinks=False):
            if any((pathlib.Path(parent) / name).is_symlink() for name in dirs):
                raise p.Error("phase_output_directory_substituted")
            for name in sorted(files):
                count += 1
                if count > 512:
                    raise p.Error("phase_output_inventory_exceeds_bound")
                path = pathlib.Path(parent) / name
                data = p.read_regular(path, v1.MAX_SOURCE_FILE_BYTES)
                if path.suffix == ".js":
                    self.artifacts.append({"path": label + "/" + str(path.relative_to(directory)),
                                           "bytes": len(data), "sha256": p.digest(data),
                                           "base64": base64.b64encode(data).decode()})
        if len(p.canonical(self.artifacts)) > MAX_EVIDENCE_BYTES:
            raise p.Error("evidence_capacity_exceeded")

    def score(self, task_id, *, mutant=False):
        # No source_root or candidate bytes argument; only fixed approved corpus.
        p.source_snapshot()
        task = next((row for row in self.tasks if row["id"] == task_id), None)
        if task is None:
            raise p.Error("task_not_in_approved_inventory")
        self.deadline = time.monotonic() + v1.MAX_TIMEOUT_SECONDS
        paths = task["languages"]["typescript"]
        parent = self.root / (task_id + ("-mutant" if mutant else "-positive"))
        parent.mkdir(mode=0o700)
        public, hidden = parent / "public", parent / "hidden"
        observations_start = len(self.authority.commands)
        record = {"task_id": task_id, "adapter_id": "typescript", "classification": "official_toolchain_conformance",
                  "mutant": mutant, "source_manifest_sha256": p.SOURCE_HASH}
        try:
            self._copy_phase(public, paths["public"])
            problem = self.scorer.hidden_overlay_problem(self.host_sources / paths["public"], self.host_sources / paths["hidden"])
            if problem:
                raise p.Error("hidden_overlay_refused:" + problem)
            if mutant:
                record["mutation"] = self._mutate(public, self.mutants[task_id])
            hidden_only = self.scorer.relative_files(self.host_sources / paths["hidden"]) - self.scorer.relative_files(self.host_sources / paths["public"])
            record["public"] = self._stage(public)
            self._capture(public, task_id + ("/mutant/public" if mutant else "/positive/public"))
            record["leak_check"] = "ok" if not hidden_only.intersection(self.scorer.relative_files(public)) else "failed"
            if record["leak_check"] != "ok":
                raise p.Error("hidden_path_leaked_into_public")
            if not mutant and not record["public"]["passed"]:
                record["status"] = "failed"
                self.results.append(record)
                return record
            self._copy_phase(hidden, paths["public"])
            for name, data in self.sources.items():
                prefix = paths["hidden"] + "/"
                if name.startswith(prefix):
                    destination = hidden / name[len(prefix):]
                    if destination.exists():
                        destination.unlink()
                    v1._write_snapshot_file(destination, data)
            if mutant:
                self._mutate(hidden, self.mutants[task_id])
            record["hidden"] = self._stage(hidden)
            self._capture(hidden, task_id + ("/mutant/hidden" if mutant else "/positive/hidden"))
            record["status"] = "ok" if record["public"]["passed"] and record["hidden"]["passed"] else "failed"
            if mutant:
                expected = self.mutants[task_id]
                if (record["public"]["phase"] != "run" or record["hidden"]["phase"] != "run"
                        or record["public"]["passed"] != expected["public_passed"]
                        or record["hidden"]["passed"] != expected["hidden_passed"]):
                    raise p.Error("mutant_did_not_prove_expected_runtime_divergence")
            self.results.append(record)
            return record
        finally:
            record["command_indices"] = list(range(observations_start, len(self.authority.commands)))
            shutil.rmtree(parent)

    def evidence(self):
        # Preserve exact command streams and policy bytes once as artifacts;
        # metadata references them rather than duplicating large SBPL strings.
        artifacts = list(self.artifacts) + self.provenance["receipt_artifacts"]
        provenance = {key: value for key, value in self.provenance.items() if key != "receipt_artifacts"}
        provenance["original_receipt_artifacts"] = [row["path"] for row in self.provenance["receipt_artifacts"]]
        inventory = [dict(row, v3_availability="admitted" if row["adapter_id"] == "typescript" else "unavailable",
                          v3_reason=None if row["adapter_id"] == "typescript" else (
                              row["blocked_reason"] or "adapter_not_admitted_under_runnable_adapter_v3"))
                     for row in self.manifest["comparison_inventory"]]
        command_rows = []
        policy_names = set()
        def artifact(name, data):
            artifacts.append({"path": name, "bytes": len(data), "sha256": p.digest(data),
                              "base64": base64.b64encode(data).decode()})
            return name
        for index, row in enumerate(self.authority.commands):
            policy = row["sandbox_argv"][2].encode()
            policy_name = "policies/" + p.digest(policy) + ".sb"
            if policy_name not in policy_names:
                artifact(policy_name, policy)
                policy_names.add(policy_name)
            command_rows.append({"argv": row["argv"], "sandbox_executable": row["sandbox_argv"][0],
                                 "sandbox_profile_artifact": policy_name,
                                 "cwd": row["cwd"], "env": row["env"], "status": row["status"], "failure": row["failure"],
                                 "stdout_artifact": artifact(f"commands/{index}/stdout.txt", base64.b64decode(row["stdout_base64"])),
                                 "stderr_artifact": artifact(f"commands/{index}/stderr.txt", base64.b64decode(row["stderr_base64"]))})
        result = {"schema": SCHEMA, "status": "official_conformance_observed", "source_origin": p.SOURCE_COMMIT,
                  "source_manifest_sha256": p.SOURCE_HASH, "execution_subject": self.subject,
                  "host": self.host, "provenance": provenance, "authority": self.observations,
                  "comparison_inventory": inventory, "results": self.results,
                  "commands": command_rows}
        if len(p.canonical(result)) > v1.MAX_RESULT_BYTES:
            raise p.Error("result_metadata_exceeds_bound")
        bundle = {"result": result, "source_manifest": self.manifest, "artifacts": artifacts}
        if len(p.canonical(bundle)) > MAX_EVIDENCE_BYTES:
            raise p.Error("evidence_capacity_exceeded")
        return bundle


def deliver(path, bundle):
    data = p.canonical(bundle)
    if len(data) > MAX_EVIDENCE_BYTES:
        raise p.Error("evidence_capacity_exceeded")
    path = pathlib.Path(path)
    if not path.is_absolute() or path.parent != path.parent.resolve() or path.name in ("", ".", ".."):
        raise p.Error("output_authority_refused")
    # Caller explicitly authorizes this existing parent; no directory creation
    # or symlink fallback. Exclusive output never overwrites evidence.
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        output = os.open(path.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=fd)
        try:
            with os.fdopen(output, "wb") as file:
                file.write(data)
        except Exception:
            os.unlink(path.name, dir_fd=fd)
            raise
    finally:
        os.close(fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provenance-directory", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    session = OfficialSession(args.provenance_directory)
    try:
        with session:
            for task in session.tasks:
                session.score(task["id"])
            bundle = session.evidence()
            deliver(args.output, bundle)
            return 0 if all(row["status"] == "ok" for row in session.results) else 1
    except (p.Error, OSError, ValueError) as error:
        failure = unavailable(str(error), getattr(error, "official_observations", session.observations))
        failure["commands"] = getattr(error, "official_commands", session.authority.commands if session.authority else [])
        # Preserve bounded authentic failure streams before reporting failure;
        # delivery failure never changes the original failure classification.
        try:
            deliver(args.output, {"result": failure})
        except (p.Error, OSError) as delivery_error:
            failure["evidence_delivery_failure"] = str(delivery_error)
        print(json.dumps({key: value for key, value in failure.items() if key != "commands"}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
