#!/usr/bin/env python3
import base64
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock
from opencode_agent_task_pilot import eligibility as elig
from opencode_agent_task_pilot.evidence import stream_provider_usage
from opencode_agent_task_pilot.replay import decode_sources
from opencode_agent_task_pilot.review_workflow import (
    audit_cohort, prepare_review_packet, FROZEN_MANIFEST_SHA256,
    FROZEN_TASK_SHA256, FROZEN_FIXTURE_SHA256,
)
from opencode_agent_task_pilot import review_workflow as workflow

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location(
    "pilot", ROOT / "scripts/opencode-agent-task-pilot.py"
)
pilot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pilot)


class PilotTests(unittest.TestCase):
    def test_frozen_pilot_input_constants_match_repository(self):
        manifest = ROOT / "benchmarks/agent-task-comparison-v1/manifest.json"
        self.assertEqual(hashlib.sha256(manifest.read_bytes()).hexdigest(), FROZEN_MANIFEST_SHA256)
        for task_path in sorted((ROOT / "benchmarks/agent-task-comparison-v1/tasks").glob("*.json")):
            task = json.loads(task_path.read_text())
            self.assertEqual(hashlib.sha256(task_path.read_bytes()).hexdigest(), FROZEN_TASK_SHA256[task["id"]])
        for fixture_name, task_id in (("fixture", "signature-migration-v1"), ("owned-fixture", "owned-signature-migration-v1")):
            inventory = {}
            fixture = ROOT / "benchmarks/agent-task-comparison-v1" / fixture_name
            for path in sorted(p for p in fixture.rglob("*") if p.is_file()):
                inventory[str(path.relative_to(fixture))] = hashlib.sha256(path.read_bytes()).hexdigest()
            digest = hashlib.sha256(json.dumps(inventory, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            self.assertEqual(digest, FROZEN_FIXTURE_SHA256[task_id])
    def test_replay_source_archive_rejects_escape_and_changed_bytes(self):
        body = b"fn main() {}"
        row = {"base64": base64.b64encode(body).decode(), "bytes": len(body),
               "sha256": hashlib.sha256(body).hexdigest()}
        self.assertEqual(decode_sources({"src/main.spx": row}), {"src/main.spx": body})
        for path in ("../main.spx", "/main.spx", "src/../main.spx", "src//main.spx"):
            with self.assertRaises(ValueError):
                decode_sources({path: row})
        with self.assertRaises(ValueError):
            decode_sources({"src/main.spx": dict(row, sha256="0" * 64)})

    def test_policy_denies_network_subagents_and_external_paths(self):
        config = pilot.policy()
        self.assertEqual(config["model"], pilot.MODEL)
        self.assertEqual(config["agent"][pilot.AGENT]["model"], pilot.MODEL)
        permissions = config["agent"][pilot.AGENT]["permission"]
        self.assertEqual(permissions["webfetch"], "deny")
        self.assertEqual(permissions["task"], "deny")
        self.assertEqual(permissions["external_directory"], "deny")

    def test_graph_lane_denies_native_source_tools(self):
        permissions = pilot.policy("semaprax-graph-operational")["agent"][pilot.AGENT]["permission"]
        for tool in ("read", "glob", "grep", "list", "edit"):
            self.assertEqual(permissions[tool], "deny")
        self.assertEqual(permissions["bash"], {"*": "deny"})
        self.assertEqual(permissions["semaprax_*"], "allow")

    def test_new_evidence_required(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(pilot.PilotFailure):
                pilot.run_tuple(
                    "signature-migration-v1",
                    "semaprax-source-first",
                    1,
                    "unused",
                    str(Path(sys.executable).resolve(strict=True)),
                    Path(temp),
                )

    def test_archived_provider_counters_do_not_estimate_tokens(self):
        body = (ROOT / "scripts/fixtures/opencode-provider-smoke-v1/session.json").read_bytes()
        counters = pilot.provider_usage(body, pilot.MODEL)
        self.assertEqual(counters["status"], "observed")
        self.assertEqual((counters["model_input_tokens"], counters["model_output_tokens"]), (4437, 214))

    def test_exported_usage_keeps_cached_and_reasoning_tokens(self):
        exported = json.loads((ROOT / "scripts/fixtures/opencode-provider-smoke-v1/session.json").read_bytes())
        tokens = exported["messages"][1]["info"]["tokens"]
        tokens["cache"] = {"read": 7, "write": 3}
        usage = pilot.provider_usage(json.dumps(exported).encode(), pilot.MODEL)
        self.assertEqual((usage["model_input_tokens"], usage["model_output_tokens"]), (4447, 214))
        del tokens["reasoning"]
        self.assertEqual(pilot.provider_usage(json.dumps(exported).encode(), pilot.MODEL)["status"], "unavailable")

    def test_stream_provider_usage_binds_each_finish_to_session_and_model(self):
        session = "ses_observed"
        event = lambda step, message, input_tokens, output_tokens: {
            "type": "step_finish", "sessionID": session,
            "part": {"type": "step-finish", "id": step, "messageID": message,
                     "sessionID": session,
                     "tokens": {"input": input_tokens, "output": output_tokens,
                                "reasoning": 2, "cache": {"read": 3, "write": 1}}},
        }
        body = b"\n".join(json.dumps(item).encode() for item in (
            {"type": "step_start", "sessionID": session, "part": {}},
            event("prt_one", "msg_one", 7, 3), event("prt_two", "msg_two", 11, 5),
        ))
        counters = stream_provider_usage(body, session, pilot.MODEL)
        self.assertEqual(counters["status"], "observed")
        self.assertEqual(counters["method"], "configured-model CLI stream report")
        self.assertEqual((counters["model_input_tokens"], counters["model_output_tokens"]), (26, 12))
        duplicate = stream_provider_usage(
            body + b"\n" + json.dumps(event("prt_two", "msg_three", 1, 1)).encode(),
            session, pilot.MODEL,
        )
        self.assertEqual(duplicate["status"], "unavailable")
        wrong_session = stream_provider_usage(body, "ses_other", pilot.MODEL)
        self.assertEqual(wrong_session["status"], "unavailable")

    def test_gateway_counter_excludes_harness_drift(self):
        event = lambda argv, out, err, code: {
            "argv_b64": pilot.base64.b64encode(argv).decode(),
            "stdout_b64": pilot.base64.b64encode(out).decode(),
            "stderr_b64": pilot.base64.b64encode(err).decode(),
            "returncode": code,
        }
        body = b"\n".join(
            json.dumps(item).encode()
            for item in (event(b"graph\0src/core.spx", b"{}", b"", 0), event(b"pilot-drift\0graph", b"", b"", 0))
        )
        diagnostics = pilot.gateway_diagnostics(body)
        self.assertEqual(diagnostics["gateway_invocations"], 1)
        self.assertEqual(diagnostics["gateway_argv_bytes"], len(b"graph\0src/core.spx"))


class SubprocessBoundaryTests(unittest.TestCase):
    def test_lane_help_explains_exact_artifact_and_source_write_routes(self):
        for lane, expected in [("semaprax-graph-operational", b".pilot/name.json"),
                               ("semaprax-source-first", b"expected lowercase sha256")]:
            with self.subTest(lane=lane), tempfile.TemporaryDirectory(prefix="spx-help-test-") as temp:
                state = Path(temp)
                candidate = state / "candidate"
                candidate.mkdir()
                compiler = state / "compiler"
                compiler.write_text("#!/bin/sh\nexit 75\n")
                compiler.chmod(0o700)
                gateway, _, _, configuration = pilot.install_gateway(compiler, state, candidate, lane)
                completed = subprocess.run([gateway, "--help"],
                    env=dict(os.environ, SEMAPRAX_PILOT_GATEWAY=configuration),
                    capture_output=True, check=False)
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertIn(expected, completed.stdout)
                self.assertIn(b"<command> --help", completed.stdout)

    def test_gateway_rejects_embedded_absolute_output_path(self):
        with tempfile.TemporaryDirectory(prefix="spx-gateway-test-") as temp:
            state = Path(temp)
            candidate = state / "candidate"
            (candidate / "src").mkdir(parents=True)
            (candidate / "src/core.spx").write_text("module sample;\n")
            # install_gateway renames its `compiler` argument in place and
            # overwrites that exact path with the wrapper text. Passing the
            # live sys.executable directly here would rename and overwrite the
            # real interpreter running this test suite; provision_semaprax
            # first copies it into disposable private state instead.
            gateway, _, _, configuration = pilot.install_gateway(
                pilot.provision_semaprax(Path(sys.executable).resolve(strict=True), state)[0],
                state, candidate, "semaprax-graph-operational",
            )
            environment = dict(os.environ, SEMAPRAX_PILOT_GATEWAY=configuration)
            completed = subprocess.run(
                [gateway, "graph", "--output=/tmp/pilot-escape"],
                env=environment, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(completed.returncode, 126)
            self.assertIn(b"option path escapes candidate", completed.stderr)

    def test_source_write_refuses_a_stale_precondition(self):
        with tempfile.TemporaryDirectory(prefix="spx-gateway-test-") as temp:
            state = Path(temp)
            candidate = state / "candidate"
            target = candidate / "src/core.spx"
            target.parent.mkdir(parents=True)
            target.write_text("module before;\n")
            gateway, _, _, configuration = pilot.install_gateway(
                pilot.provision_semaprax(Path(sys.executable).resolve(strict=True), state)[0],
                state, candidate, "semaprax-source-first",
            )
            environment = dict(os.environ, SEMAPRAX_PILOT_GATEWAY=configuration)
            stale_digest = pilot.sha(target.read_bytes())
            target.write_text("module injected;\n")
            completed = subprocess.run(
                [gateway, "pilot-write-source", "src/core.spx", stale_digest,
                 pilot.base64.b64encode(b"module overwrite;\n").decode()],
                env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(completed.returncode, 126)
            self.assertEqual(target.read_text(), "module injected;\n")
            self.assertIn(b"precondition is stale", completed.stderr)

    def test_gateway_primary_records_prove_a_rejected_stale_recovery(self):
        with tempfile.TemporaryDirectory(prefix="spx-gateway-drift-test-") as temp:
            state = Path(temp)
            candidate = state / "candidate"
            target = candidate / "src/core.spx"
            target.parent.mkdir(parents=True)
            before = b"module before;\n"
            after = b"module after;\n"
            target.write_bytes(before)
            drift = state / "drift.json"
            drift.write_text(json.dumps({
                "target": "src/core.spx",
                "before_b64": base64.b64encode(before).decode(),
                "after_b64": base64.b64encode(after).decode(),
                "before_sha256": hashlib.sha256(before).hexdigest(),
                "after_sha256": hashlib.sha256(after).hexdigest(),
                "applied": False,
            }, sort_keys=True))
            gateway, _, log, configuration = pilot.install_gateway(
                pilot.provision_semaprax(Path(sys.executable).resolve(strict=True), state)[0],
                state, candidate, "semaprax-source-first", drift,
            )
            environment = dict(os.environ, SEMAPRAX_PILOT_GATEWAY=configuration)
            read = subprocess.run(
                [gateway, "pilot-read", "src/core.spx"], env=environment,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(read.returncode, 0, read.stderr)
            rejected = subprocess.run(
                [gateway, "pilot-write-source", "src/core.spx", hashlib.sha256(before).hexdigest(),
                 base64.b64encode(b"module replacement;\n").decode()],
                env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(rejected.returncode, 126)
            self.assertEqual(target.read_bytes(), after)
            metric = elig.stale_recovery_events(log.read_bytes(), True)
            self.assertEqual(metric["status"], "observed")
            self.assertEqual(metric["events"], [{
                "trigger": "drift_on_source_read", "recovery_outcome": "rejected_stale_write",
            }])

    def test_output_cap_kills_continuous_writer(self):
        old = pilot.CAP
        pilot.CAP = 1024
        try:
            with self.assertRaisesRegex(pilot.PilotFailure, "output cap"):
                pilot.bounded(
                    [
                        sys.executable,
                        "-c",
                        "import sys\nwhile True: sys.stdout.write('x'*512);sys.stdout.flush()",
                    ],
                    ROOT,
                    2,
                )
        finally:
            pilot.CAP = old

    def test_closed_pipes_still_obey_timeout(self):
        with self.assertRaisesRegex(pilot.PilotFailure, "timed out"):
            pilot.bounded(
                [
                    sys.executable,
                    "-c",
                    "import os,time;os.close(1);os.close(2);time.sleep(2)",
                ],
                ROOT,
                0.02,
            )

    def test_exited_parent_cannot_leave_quiet_pipe_descendant(self):
        with tempfile.TemporaryDirectory(prefix="spx-descendant-test-") as temp:
            pid_file = Path(temp) / "pid"
            child_code = f"import os, pathlib, time; pathlib.Path({str(pid_file)!r}).write_text(str(os.getpid())); time.sleep(5)"
            code = f"import subprocess, sys, time; subprocess.Popen([sys.executable, '-c', {child_code!r}]); time.sleep(0.1); print('parent done')"
            start = time.monotonic()
            pilot.bounded([sys.executable, "-c", code], ROOT, 2)
            self.assertLess(time.monotonic() - start, 1)
            for _ in range(20):
                if pid_file.exists():
                    break
                time.sleep(0.01)
            self.assertTrue(pid_file.exists())
            pid = int(pid_file.read_text())
            for _ in range(50):
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.01)
            else:
                self.fail("quiet-pipe descendant survived process-group cleanup")

    def test_compiler_source_must_be_regular_non_symlink_executable(self):
        with tempfile.TemporaryDirectory(prefix="spx-compiler-test-") as temp:
            root = Path(temp)
            nonexec = root / "nonexec"
            nonexec.write_text("compiler")
            with self.assertRaisesRegex(pilot.PilotFailure, "regular executable"):
                pilot.provision_semaprax(nonexec, root / "state")
            link = root / "link"
            link.symlink_to(Path(sys.executable).resolve(strict=True))
            with self.assertRaisesRegex(pilot.PilotFailure, "non-symlink"):
                pilot.provision_semaprax(link, root / "state")

    @unittest.skipUnless(shutil.which("sandbox-exec"), "sandbox-exec unavailable")
    def test_saved_profile_denies_directory_descendants_and_outside_write(self):
        with tempfile.TemporaryDirectory(prefix="spx-profile-test-") as temp:
            root = Path(temp).resolve()
            candidate = root / "candidate"
            protected = root / "protected"
            state = root / "state"
            for directory in (candidate, protected, state):
                directory.mkdir()
            allowed = candidate / "fixture.txt"
            hidden = protected / "nested" / "rubric.json"
            hidden.parent.mkdir()
            allowed.write_text("candidate")
            hidden.write_text("rubric")
            profile = state / "seatbelt.sb"
            profile.write_text(pilot.seatbelt_profile(candidate, (protected,), state))
            pilot.seatbelt_probe(profile, allowed, (hidden,), candidate, state)
            self.assertFalse((candidate / ".seatbelt-probe").exists())
            self.assertFalse((state / ".seatbelt-probe").exists())

    @unittest.skipUnless(shutil.which("sandbox-exec"), "sandbox-exec unavailable")
    def test_probe_rejects_the_old_directory_literal_rule(self):
        with tempfile.TemporaryDirectory(prefix="spx-old-profile-test-") as temp:
            root = Path(temp).resolve()
            candidate = root / "candidate"
            protected = root / "protected"
            state = root / "state"
            for directory in (candidate, protected, state):
                directory.mkdir()
            allowed = candidate / "fixture.txt"
            hidden = protected / "rubric.json"
            allowed.write_text("candidate")
            hidden.write_text("rubric")
            profile = state / "old-seatbelt.sb"
            profile.write_text(
                "(version 1)\n(allow default)\n"
                f'(deny file-read* (literal "{protected}"))\n'
                f'(deny file-write* (require-not (require-any (subpath "{candidate}") (subpath "{state}"))))\n'
            )
            with self.assertRaisesRegex(
                pilot.PilotFailure, "read protected descendant"
            ):
                pilot.seatbelt_probe(profile, allowed, (hidden,), candidate, state)


class TupleTransportTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which("sandbox-exec"), "sandbox-exec unavailable")
    def test_source_and_graph_wrapped_runs_archive_their_boundary_proof(self):
        with tempfile.TemporaryDirectory(prefix="spx-stub-test-") as temp:
            root = Path(temp).resolve()
            evidence_parent = root / "evidence-parent"
            evidence_parent.mkdir()
            hidden = evidence_parent / "hidden-rubric.json"
            hidden.write_text("rubric")
            outside = root / "outside"
            outside.write_text("unchanged")
            originals = [
                ROOT / "benchmarks/agent-task-comparison-v1/manifest.json",
                pilot.original_repository_root()
                / "benchmarks/agent-task-comparison-v1/manifest.json",
                hidden,
            ]
            stub = root / "opencode-stub"
            stub.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, shutil, subprocess, sys\n"
                "from pathlib import Path\n"
                f"protected={list(map(str, originals))!r}\n"
                f"outside=Path({str(outside)!r})\n"
                "args=sys.argv[1:]\n"
                "phase=args[0]\n"
                "assert phase in ('run','export')\n"
                "assert '--pure' in args\n"
                "assert 'PILOT_TEST_SECRET' not in os.environ\n"
                "config=Path(os.environ['OPENCODE_CONFIG'])\n"
                "assert config.is_absolute() and config.is_file()\n"
                "assert json.loads(config.read_text())['agent']\n"
                "assert Path(os.environ['HOME']).parent == config.parent\n"
                "assert all(Path(os.environ[x]).parent == config.parent for x in ('XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_CACHE_HOME','TMPDIR'))\n"
                "compiler=shutil.which('semaprax')\n"
                "assert compiler and Path(compiler).parent == config.parent/'bin'\n"
                "mcp_command=json.loads(config.read_text())['mcp']['semaprax']['command']\n"
                "assert Path(mcp_command[2]) == Path(compiler), 'MCP must run the gateway executable, never its log'\n"
                "rpc=json.dumps({'jsonrpc':'2.0','id':1,'method':'tools/list'})+'\\n'\n"
                "probe=subprocess.run(mcp_command,input=rpc,text=True,capture_output=True,timeout=5)\n"
                "assert probe.returncode == 0 and json.loads(probe.stdout)['result']['tools'][0]['name'] == 'command'\n"
                "rpc=json.dumps({'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'command','arguments':{'argv':['--version']}}})+'\\n'\n"
                "probe=subprocess.run(mcp_command,input=rpc,text=True,capture_output=True,timeout=5)\n"
                "result=json.loads(probe.stdout)['result']\n"
                "assert probe.returncode == 0 and not result['isError'], result\n"
                "for item in protected:\n"
                "    try: Path(item).read_bytes()\n"
                "    except OSError: pass\n"
                "    else: raise SystemExit('protected read succeeded: '+item)\n"
                "try: outside.write_text('escaped')\n"
                "except OSError: pass\n"
                "else: raise SystemExit('outside write succeeded')\n"
                "if phase == 'run':\n"
                f"    assert args[args.index('--model')+1] == {pilot.MODEL!r}\n"
                "    candidate=Path(args[args.index('--dir')+1])\n"
                "    assert (candidate/'semaprax.toml').is_file()\n"
                "    (candidate/'agent-proof').write_text('run was confined')\n"
                "    print(json.dumps({'sessionID':'stub-session','boundary':'enforced'}))\n"
                "else:\n"
                "    assert args[1] == 'stub-session'\n"
                "    (config.parent/'export-proof').write_text('export was confined')\n"
                "    print(json.dumps({'phase':'export','boundary':'enforced'}))\n"
            )
            stub.chmod(0o700)
            with mock.patch.dict(os.environ, {"PILOT_TEST_SECRET": "must-not-pass"}):
                compiler_source = root / "compiler-stub"
                compiler_source.write_text("#!/usr/bin/python3\nprint('compiler stub')\n")
                compiler_source.chmod(0o700)
                compiler_before = compiler_source.read_bytes()
                protocol = {
                    "schema": workflow.PROTOCOL_SCHEMA,
                    "id": "stub-transport-v1",
                    "manifest_sha256": FROZEN_MANIFEST_SHA256,
                    "runner_revision": "a" * 40,
                    "budget_policy": "test-budget", "egress_policy": "test-egress",
                    "models": [
                        {"id": "stub-model", "provider": "opencode", "model": "muse-spark-1.3-contributor-free",
                         "revision": "1.3", "configured_model": pilot.MODEL,
                         "tokenizer": "test-tokenizer", "configuration": "test-config"},
                        {"id": "other-model", "provider": "other", "model": "other-model",
                         "revision": "1", "configured_model": "other/other-model",
                         "tokenizer": "test-tokenizer", "configuration": "test-config"},
                    ],
                }
                protocol_path = root / "protocol.json"
                protocol_path.write_bytes(json.dumps(protocol, sort_keys=True, separators=(",", ":")).encode() + b"\n")
                records = {
                    lane: pilot.run_tuple(
                        "signature-migration-v1", lane, 1, stub,
                        str(compiler_source), evidence_parent / lane, 10, protocol_path, "stub-model",
                    )
                    for lane in ("semaprax-source-first", "semaprax-graph-operational")
                }
            record = records["semaprax-source-first"]
            evidence = evidence_parent / "semaprax-source-first"
            self.assertEqual(compiler_source.read_bytes(), compiler_before)
            self.assertEqual(record["semaprax_sha256"], pilot.sha(compiler_before))
            self.assertEqual(record["status"], "ineligible")
            self.assertEqual(outside.read_text(), "unchanged")
            self.assertEqual(
                json.loads((evidence / "stdout.jsonl").read_text())["boundary"],
                "enforced",
            )
            self.assertEqual(
                json.loads((evidence / "session.json").read_text()),
                {"phase": "export", "boundary": "enforced"},
            )
            self.assertIn("agent-proof", record["after"])
            self.assertNotIn("agent-proof", record["before"])
            self.assertIn("subpath", (evidence / "seatbelt.sb").read_text())
            self.assertEqual(
                record["stdout_sha256"],
                pilot.sha((evidence / "stdout.jsonl").read_bytes()),
            )
            for lane, lane_record in records.items():
                lane_evidence = evidence_parent / lane
                self.assertEqual(lane_record["status"], "ineligible")
                self.assertTrue((lane_evidence / "candidate-source.json").is_file())
                self.assertTrue((lane_evidence / "mcp-wire.jsonl").is_file())


def _gateway_event(argv, code=0, out=b"{}", err=b""):
    return json.dumps({
        "argv_b64": base64.b64encode("\0".join(argv).encode()).decode(),
        "stdout_b64": base64.b64encode(out).decode(),
        "stderr_b64": base64.b64encode(err).decode(),
        "returncode": code,
    })


def _gateway_log(*events):
    return ("\n".join(events) + "\n").encode() if events else b""


def _drift_event(command, before=b"before", after=b"after"):
    return _gateway_event(
        ["pilot-drift", command],
        out=json.dumps({
            "after_sha256": hashlib.sha256(after).hexdigest(),
            "before_sha256": hashlib.sha256(before).hexdigest(),
            "trigger": command,
        }, sort_keys=True).encode(),
    )


def _mcp_frame(response_bytes=100):
    request = json.dumps({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "command", "arguments": {"argv": ["--version"]}},
    }).encode()
    response = json.dumps({
        "jsonrpc": "2.0", "id": 1,
        "result": {"content": [{"type": "text", "text": "x" * max(0, response_bytes - 60)}], "isError": False},
    }).encode()
    return json.dumps({
        "request_b64": base64.b64encode(request).decode(),
        "response_b64": base64.b64encode(response).decode(),
    })


class PresentedContextBytesTests(unittest.TestCase):
    def _write(self, evidence, prompt="do the migration"):
        wire = (_mcp_frame(250) + "\n").encode()
        (evidence / "mcp-wire.jsonl").write_bytes(wire)
        manifest = elig.write_presentation_evidence(evidence, prompt, wire)
        return wire, manifest

    def test_binds_prompt_and_raw_mcp_tool_response_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = Path(temp)
            wire, manifest = self._write(evidence)
            result = elig.presented_context_bytes(evidence, "do the migration")
            self.assertEqual(result["status"], "observed")
            self.assertEqual(result["prompt_bytes"], len("do the migration".encode("utf-8")))
            self.assertEqual(result["mcp_wire_sha256"], hashlib.sha256(wire).hexdigest())
            self.assertEqual(result["presentation_sha256"], manifest["presentation_sha256"])
            self.assertEqual(result["presented_context_bytes"], result["prompt_bytes"] + result["tool_response_bytes"])

    def test_missing_or_noncanonical_presentation_evidence_is_unavailable(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = Path(temp)
            (evidence / "mcp-wire.jsonl").write_bytes(b"")
            self.assertEqual(elig.presented_context_bytes(evidence, "prompt")["status"], "unavailable")
            self._write(evidence, "prompt")
            raw = json.loads((evidence / "presentation.json").read_text())
            (evidence / "presentation.json").write_text(json.dumps(raw, indent=2) + "\n")
            result = elig.presented_context_bytes(evidence, "prompt")
            self.assertEqual(result["status"], "unavailable")
            self.assertIn("canonical", result["reason"])

    def test_changed_prompt_or_wire_cannot_reuse_presentation_record(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = Path(temp)
            self._write(evidence, "prompt")
            self.assertEqual(elig.presented_context_bytes(evidence, "changed")["status"], "unavailable")
            (evidence / "mcp-wire.jsonl").write_bytes(b"")
            result = elig.presented_context_bytes(evidence, "prompt")
            self.assertEqual(result["status"], "unavailable")
            self.assertIn("differs", result["reason"])

    def test_exclusive_no_follow_creation_refuses_existing_link(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = Path(temp)
            outside = Path(temp).parent / "presentation-outside.json"
            (evidence / "presentation.json").symlink_to(outside)
            with self.assertRaises(ValueError):
                elig.write_presentation_evidence(evidence, "prompt", b"")

    def test_link_backed_presentation_record_is_unavailable(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = Path(temp)
            self._write(evidence, "prompt")
            raw = (evidence / "presentation.json").read_bytes()
            outside = Path(temp).parent / "presentation-reader-outside.json"
            outside.write_bytes(raw)
            (evidence / "presentation.json").unlink()
            (evidence / "presentation.json").symlink_to(outside)
            result = elig.presented_context_bytes(evidence, "prompt")
            self.assertEqual(result["status"], "unavailable")


class StaleRecoveryEventsTests(unittest.TestCase):
    def test_task_without_drift_scenario_shows_zero_triggers(self):
        result = elig.stale_recovery_events(b"", False)
        self.assertEqual(result, {"status": "observed", "stale_failures": 0, "stale_recovery_actions": 0, "events": []})

    def test_conditional_write_after_trigger_is_recovered(self):
        body = _gateway_log(
            _gateway_event(["pilot-read", "src/core.spx"], out=b"before"),
            _drift_event("pilot-read"),
            _gateway_event(
                ["pilot-write-source", "src/core.spx", hashlib.sha256(b"after").hexdigest(), "Zm9v"],
                code=0, out=b"source written\n",
            ),
        )
        result = elig.stale_recovery_events(body, True)
        self.assertEqual(result["status"], "observed")
        self.assertEqual(result["stale_failures"], 1)
        self.assertEqual(result["stale_recovery_actions"], 1)
        self.assertEqual(result["events"][0]["trigger"], "drift_on_source_read")
        self.assertEqual(result["events"][0]["recovery_outcome"], "recovered_conditional_write")

    def test_rejected_stale_write_after_identifying_trigger(self):
        body = _gateway_log(
            _drift_event("graph"),
            _gateway_event(
                ["pilot-write-source", "src/core.spx", hashlib.sha256(b"before").hexdigest(), "Zm9v"],
                code=126, out=b"",
                err=b"pilot-write-source precondition is stale\n",
            ),
        )
        result = elig.stale_recovery_events(body, True)
        self.assertEqual(result["events"][0]["trigger"], "drift_on_identifying_command")
        self.assertEqual(result["events"][0]["recovery_outcome"], "rejected_stale_write")
        self.assertEqual(result["stale_recovery_actions"], 0)

    def test_drift_receipt_and_source_read_are_primary_evidence(self):
        receipt = json.loads(base64.b64decode(json.loads(_drift_event("pilot-read"))["stdout_b64"]))
        receipt["before_sha256"] = hashlib.sha256(b"substituted").hexdigest()
        malformed_receipt = _gateway_event(
            ["pilot-drift", "pilot-read"], out=json.dumps(receipt, sort_keys=True).encode(),
        )
        for body in (
            _gateway_log(_gateway_event(["pilot-read", "src/core.spx"], out=b"before"), malformed_receipt),
            _gateway_log(_gateway_event(["pilot-read", "src/core.spx"], out=b"before"),
                         _gateway_event(["pilot-drift", "graph"], code=126)),
        ):
            with self.subTest(body=body):
                result = elig.stale_recovery_events(body, True)
                self.assertEqual(result["status"], "unavailable")

    def test_recovery_write_must_bind_the_drift_source_revisions(self):
        for write in (
            _gateway_event(
                ["pilot-write-source", "src/core.spx", hashlib.sha256(b"before").hexdigest(), "Zm9v"],
                out=b"source written\n",
            ),
            _gateway_event(
                ["pilot-write-source", "src/core.spx", hashlib.sha256(b"after").hexdigest(), "Zm9v"],
                code=126, err=b"pilot-write-source precondition is stale\n",
            ),
            _gateway_event(
                ["pilot-write-source", "src/core.spx", hashlib.sha256(b"after").hexdigest(), "Zm9v"],
                out=b"unexpected success\n",
            ),
        ):
            with self.subTest(write=write):
                body = _gateway_log(_drift_event("graph"), write)
                result = elig.stale_recovery_events(body, True)
                self.assertEqual(result["status"], "unavailable")

    def test_no_recovery_attempt_when_file_never_touched_again(self):
        body = _gateway_log(_drift_event("graph"))
        result = elig.stale_recovery_events(body, True)
        self.assertEqual(result["events"][0]["recovery_outcome"], "no_recovery_attempt")

    def test_later_read_of_drifted_file_is_not_a_false_recovery(self):
        body = _gateway_log(
            _gateway_event(["pilot-read", "src/core.spx"], out=b"before"),
            _drift_event("pilot-read"),
            _gateway_event(["pilot-read", "src/core.spx"], code=0),
        )
        result = elig.stale_recovery_events(body, True)
        self.assertEqual(result["status"], "observed")
        self.assertEqual(result["events"][0]["recovery_outcome"], "no_recovery_attempt")
        self.assertEqual(result["stale_recovery_actions"], 0)

    def test_malformed_synthetic_drift_event_is_unavailable(self):
        for argv in (["pilot-drift"], ["pilot-drift", "not-an-identifying-command"],
                     ["pilot-drift", "graph", "extra"]):
            with self.subTest(argv=argv):
                result = elig.stale_recovery_events(_gateway_log(_gateway_event(argv)), True)
                self.assertEqual(result["status"], "unavailable")
                self.assertIn("malformed", result["reason"])

    def test_malformed_source_recovery_write_is_unavailable(self):
        valid = ["pilot-write-source", "src/core.spx", "f" * 64, "Zm9v"]
        malformed = (
            valid[:-1],
            [*valid, "extra"],
            [valid[0], valid[1], "F" * 64, valid[3]],
            [valid[0], valid[1], valid[2], "not-base64!"],
            [valid[0], "src/app.spx", valid[2], "not-base64!"],
        )
        for argv in malformed:
            with self.subTest(argv=argv):
                body = _gateway_log(
                    _gateway_event(["pilot-read", "src/core.spx"], out=b"before"),
                    _drift_event("pilot-read"),
                    _gateway_event(argv, code=126),
                )
                result = elig.stale_recovery_events(body, True)
                self.assertEqual(result["status"], "unavailable")
                self.assertIn("malformed", result["reason"])

    def test_non_stale_source_write_failure_is_unavailable(self):
        body = _gateway_log(
            _gateway_event(["pilot-read", "src/core.spx"], out=b"before"),
            _drift_event("pilot-read"),
            _gateway_event(
                ["pilot-write-source", "src/core.spx", hashlib.sha256(b"before").hexdigest(), "Zm9v"],
                code=126,
                err=b"pilot-write-source body exceeds cap\n",
            ),
        )
        result = elig.stale_recovery_events(body, True)
        self.assertEqual(result["status"], "unavailable")
        self.assertIn("stale-precondition", result["reason"])

    def test_different_target_write_is_not_a_recovery_attempt(self):
        body = _gateway_log(
            _gateway_event(["pilot-read", "src/core.spx"], out=b"before"),
            _drift_event("pilot-read"),
            _gateway_event(["pilot-write-source", "src/app.spx", "f" * 64, "Zm9v"], code=0),
        )
        result = elig.stale_recovery_events(body, True)
        self.assertEqual(result["status"], "observed")
        self.assertEqual(result["events"][0]["recovery_outcome"], "no_recovery_attempt")

    def test_inconsistent_declared_but_no_trigger_is_unavailable(self):
        result = elig.stale_recovery_events(b"", True)
        self.assertEqual(result["status"], "unavailable")

    def test_inconsistent_trigger_but_not_declared_is_unavailable(self):
        body = _gateway_log(_drift_event("graph"))
        result = elig.stale_recovery_events(body, False)
        self.assertEqual(result["status"], "unavailable")

    def test_malformed_gateway_log_raises_value_error_not_silently_zero(self):
        with self.assertRaises(ValueError):
            elig.stale_recovery_events(b"not json\n", False)


class BlindedReviewTests(unittest.TestCase):
    def _evidence_with_diff(self, temp):
        evidence = Path(temp)
        (evidence / "candidate.diff").write_text("--- before\n+++ after\n")
        return evidence

    def test_missing_review_record_is_unavailable(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["status"], "unavailable")
            self.assertIn("no blinded review record", result["reason"])

    def test_recorded_review_round_trips_and_is_observed(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            record = elig.record_blinded_review(evidence, "reviewer-1", 1000, 2_000_000_000, 500, True)
            self.assertEqual(record["schema"], elig.REVIEW_SCHEMA)
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["status"], "observed")
            self.assertEqual(result["review_wall_ms"], 500)
            self.assertEqual(result["reviewer_id"], "reviewer-1")

    def test_second_review_record_is_refused_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            elig.record_blinded_review(evidence, "reviewer-1", 0, 2_000_000_000, 500, True)
            with self.assertRaises(FileExistsError):
                elig.record_blinded_review(evidence, "reviewer-2", 0, 2_000_000_000, 500, True)

    def test_unblinded_review_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            with self.assertRaisesRegex(ValueError, "blinded=True"):
                elig.record_blinded_review(evidence, "reviewer-1", 0, 2_000_000_000, 500, False)

    def test_active_time_exceeding_elapsed_interval_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            with self.assertRaises(ValueError):
                elig.record_blinded_review(evidence, "reviewer-1", 0, 1_000_000, 999_999_999, True)

    def test_review_bound_to_a_different_diff_is_unavailable(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            elig.record_blinded_review(evidence, "reviewer-1", 0, 2_000_000_000, 500, True)
            (evidence / "candidate.diff").write_text("--- changed\n")
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["status"], "unavailable")
            self.assertIn("different candidate diff", result["reason"])

    def test_tampered_attestation_is_unavailable_not_crash(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            elig.record_blinded_review(evidence, "reviewer-1", 0, 2_000_000_000, 500, True)
            value = json.loads((evidence / "review.json").read_text())
            value["blinded"] = False
            (evidence / "review.json").write_text(json.dumps(value))
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["status"], "unavailable")
            self.assertIn("blinded", result["reason"])

    def test_host_clock_timed_packet_session_is_the_eligible_review_shape(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            started = elig.start_blinded_review(evidence, started_ns=1_000_000_000)
            with self.assertRaisesRegex(ValueError, "artifact already exists"):
                elig.start_blinded_review(evidence, started_ns=1_000_000_001)
            review = elig.finish_blinded_review(
                evidence, "reviewer-1", "accept", True, stopped_ns=3_500_000_000,
            )
            self.assertEqual(review["review_session_sha256"], started["review_session_sha256"])
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["status"], "observed")
            self.assertEqual(result["assurance"], "timed_packet_bound")
            self.assertEqual(result["review_wall_ms"], 2500)

    def test_timed_session_rejects_stale_packet_and_tampered_duration(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            elig.start_blinded_review(evidence, started_ns=1_000_000_000)
            (evidence / "candidate.diff").write_text("--- changed\n")
            with self.assertRaisesRegex(ValueError, "stale"):
                elig.finish_blinded_review(
                    evidence, "reviewer-1", "reject", True, stopped_ns=3_000_000_000,
                )

        with tempfile.TemporaryDirectory() as temp:
            evidence = self._evidence_with_diff(temp)
            elig.start_blinded_review(evidence, started_ns=1_000_000_000)
            elig.finish_blinded_review(evidence, "reviewer-1", "reject", True, stopped_ns=3_000_000_000)
            review = json.loads((evidence / "review.json").read_text())
            review["active_ms"] -= 1
            (evidence / "review.json").write_text(json.dumps(review, sort_keys=True) + "\n")
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["status"], "unavailable")
            self.assertIn("duration", result["reason"])


class InterventionLedgerTests(unittest.TestCase):
    def test_absent_ledger_is_unavailable_not_zero(self):
        with tempfile.TemporaryDirectory() as temp:
            result = elig.intervention_ledger(temp)
            self.assertEqual(result["status"], "unavailable")

    def test_initialized_empty_ledger_is_observed_zero(self):
        with tempfile.TemporaryDirectory() as temp:
            elig.initialize_intervention_ledger(temp)
            result = elig.intervention_ledger(temp)
            self.assertEqual(result, {
                "status": "observed", "human_interventions": 0, "events": [],
                "sha256": hashlib.sha256(b"").hexdigest(),
            })

    def test_append_is_ordered_and_readable(self):
        with tempfile.TemporaryDirectory() as temp:
            elig.initialize_intervention_ledger(temp)
            first = elig.append_intervention(temp, "manual_process_kill", "pid-123", note="hung", timestamp_ns=10)
            second = elig.append_intervention(temp, "timeout_extension", "trial-timeout", timestamp_ns=20)
            self.assertEqual((first["sequence"], second["sequence"]), (1, 2))
            result = elig.intervention_ledger(temp)
            self.assertEqual(result["status"], "observed")
            self.assertEqual(result["human_interventions"], 2)
            self.assertEqual([event["kind"] for event in result["events"]],
                              ["manual_process_kill", "timeout_extension"])

    def test_unknown_kind_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            elig.initialize_intervention_ledger(temp)
            with self.assertRaises(ValueError):
                elig.append_intervention(temp, "not_a_real_kind", "target")

    def test_out_of_order_timestamp_is_refused_on_append(self):
        with tempfile.TemporaryDirectory() as temp:
            elig.initialize_intervention_ledger(temp)
            elig.append_intervention(temp, "manual_process_kill", "pid-1", timestamp_ns=100)
            with self.assertRaises(ValueError):
                elig.append_intervention(temp, "manual_process_kill", "pid-2", timestamp_ns=50)

    def test_tampered_sequence_is_unavailable_not_crash(self):
        with tempfile.TemporaryDirectory() as temp:
            elig.initialize_intervention_ledger(temp)
            elig.append_intervention(temp, "manual_process_kill", "pid-1", timestamp_ns=1)
            path = Path(temp) / "interventions.jsonl"
            entries = [json.loads(line) for line in path.read_text().splitlines()]
            entries[0]["sequence"] = 7
            path.write_text("\n".join(json.dumps(entry) for entry in entries) + "\n")
            result = elig.intervention_ledger(temp)
            self.assertEqual(result["status"], "unavailable")
            self.assertIn("append-only", result["reason"])

    def test_unknown_kind_written_directly_is_unavailable_not_crash(self):
        with tempfile.TemporaryDirectory() as temp:
            elig.initialize_intervention_ledger(temp)
            path = Path(temp) / "interventions.jsonl"
            entry = {"schema": elig.INTERVENTION_SCHEMA, "sequence": 1, "kind": "not_closed",
                      "target": "x", "timestamp_ns": 1, "note": None}
            path.write_text(json.dumps(entry) + "\n")
            result = elig.intervention_ledger(temp)
            self.assertEqual(result["status"], "unavailable")

    def test_truncated_garbage_ledger_is_unavailable_not_crash(self):
        with tempfile.TemporaryDirectory() as temp:
            (Path(temp) / "interventions.jsonl").write_text("{not json\n")
            result = elig.intervention_ledger(temp)
            self.assertEqual(result["status"], "unavailable")

    @unittest.skipUnless(elig.fcntl is not None, "POSIX advisory locks are required by the pilot")
    def test_append_waits_for_the_ledger_lock_then_keeps_sequences_unique(self):
        """A concurrent operator cannot derive a duplicate sequence from stale bytes."""
        with tempfile.TemporaryDirectory() as temp:
            path = elig.initialize_intervention_ledger(temp)
            fd = os.open(path, os.O_RDWR)
            elig.fcntl.flock(fd, elig.fcntl.LOCK_EX)
            finished = threading.Event()
            failure = []

            def append():
                try:
                    elig.append_intervention(temp, "manual_process_kill", "pid-1", timestamp_ns=1)
                except Exception as error:  # pragma: no cover - asserted below
                    failure.append(error)
                finally:
                    finished.set()

            worker = threading.Thread(target=append)
            worker.start()
            self.assertFalse(finished.wait(0.2), "append ignored the held ledger lock")
            elig.fcntl.flock(fd, elig.fcntl.LOCK_UN)
            os.close(fd)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(failure, [])
            second = elig.append_intervention(temp, "timeout_extension", "trial-timeout", timestamp_ns=2)
            self.assertEqual(second["sequence"], 2)
            self.assertEqual(
                [event["sequence"] for event in elig.intervention_ledger(temp)["events"]], [1, 2]
            )

    def test_initialize_ledger_refuses_a_preexisting_symlink(self):
        with tempfile.TemporaryDirectory() as temp:
            outside = Path(temp) / "outside"
            outside.write_text("untrusted\n")
            (Path(temp) / "interventions.jsonl").symlink_to(outside)
            with self.assertRaises(ValueError):
                elig.initialize_intervention_ledger(temp)


class ComputeEligibilityTests(unittest.TestCase):
    def _complete_evidence(self, temp, prompt="migrate the signature"):
        evidence = Path(temp)
        (evidence / "candidate.diff").write_text("--- before\n+++ after\n")
        wire = (_mcp_frame(42) + "\n").encode()
        (evidence / "mcp-wire.jsonl").write_bytes(wire)
        elig.write_presentation_evidence(evidence, prompt, wire)
        elig.initialize_intervention_ledger(evidence)
        elig.start_blinded_review(evidence, started_ns=0)
        elig.finish_blinded_review(
            evidence, "reviewer-1", "accept", True, stopped_ns=600_000_000_000,
        )
        return evidence

    def _complete_kwargs(self, evidence, drift_declared=False, gateway_log_bytes=b""):
        return dict(
            prompt="migrate the signature",
            gateway_log_bytes=gateway_log_bytes,
            drift_declared=drift_declared,
            evidence_dir=evidence,
        )

    def test_complete_record_is_eligible(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._complete_evidence(temp)
            result = elig.compute_eligibility(**self._complete_kwargs(evidence))
            self.assertTrue(result["eligible"], result["reasons"])
            self.assertEqual(result["reasons"], [])
            for key in ("presentation_bytes", "stale_recovery", "blinded_review", "intervention_ledger"):
                self.assertEqual(result[key]["status"], "observed")

    def test_each_missing_metric_is_named_and_others_stay_fine(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._complete_evidence(temp)
            # Remove only the review record.
            (evidence / "review.json").unlink()
            result = elig.compute_eligibility(**self._complete_kwargs(evidence))
            self.assertFalse(result["eligible"])
            self.assertEqual(len(result["reasons"]), 1)
            self.assertIn("blinded active review time", result["reasons"][0])
            self.assertEqual(result["presentation_bytes"]["status"], "observed")
            self.assertEqual(result["intervention_ledger"]["status"], "observed")

        with tempfile.TemporaryDirectory() as temp:
            evidence = self._complete_evidence(temp)
            (evidence / "interventions.jsonl").unlink()
            result = elig.compute_eligibility(**self._complete_kwargs(evidence))
            self.assertFalse(result["eligible"])
            self.assertIn("intervention ledger", result["reasons"][0])

        with tempfile.TemporaryDirectory() as temp:
            evidence = self._complete_evidence(temp)
            kwargs = self._complete_kwargs(evidence)
            (evidence / "presentation.json").unlink()
            result = elig.compute_eligibility(**kwargs)
            self.assertFalse(result["eligible"])
            self.assertIn("presentation bytes", result["reasons"][0])

        with tempfile.TemporaryDirectory() as temp:
            evidence = self._complete_evidence(temp)
            kwargs = self._complete_kwargs(evidence, drift_declared=True)
            result = elig.compute_eligibility(**kwargs)
            self.assertFalse(result["eligible"])
            self.assertIn("typed stale/recovery metrics", result["reasons"][0])

    def test_malformed_gateway_log_is_ineligible_not_a_crash(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._complete_evidence(temp)
            kwargs = self._complete_kwargs(evidence, gateway_log_bytes=b"{garbage")
            result = elig.compute_eligibility(**kwargs)
            self.assertFalse(result["eligible"])
            self.assertEqual(result["stale_recovery"]["status"], "unavailable")

    def test_malformed_intervention_ledger_is_ineligible_not_a_crash(self):
        with tempfile.TemporaryDirectory() as temp:
            evidence = self._complete_evidence(temp)
            (evidence / "interventions.jsonl").write_text("not json\n")
            result = elig.compute_eligibility(**self._complete_kwargs(evidence))
            self.assertFalse(result["eligible"])
            self.assertEqual(result["intervention_ledger"]["status"], "unavailable")

    def test_exception_raising_evidence_dir_never_propagates(self):
        # A path type that raises on nearly every operation must still yield a
        # clean ineligible result rather than an unhandled exception.
        class Explosive:
            def __truediv__(self, other):
                raise RuntimeError("boom")

            def __fspath__(self):
                raise RuntimeError("boom")

        result = elig.compute_eligibility(
            prompt="p",
            gateway_log_bytes=b"", drift_declared=False, evidence_dir=Explosive(),
        )
        self.assertFalse(result["eligible"])
        self.assertTrue(result["reasons"])


class RunTupleEligibilityCliTests(unittest.TestCase):
    def test_start_and_finish_review_cli_derive_host_clock_duration(self):
        with tempfile.TemporaryDirectory(prefix="spx-cli-timed-review-") as temp:
            evidence = Path(temp)
            (evidence / "candidate.diff").write_text("--- a\n+++ b\n")
            started = subprocess.run(
                [sys.executable, str(ROOT / "scripts/opencode-agent-task-pilot.py"),
                 "start-review", "--evidence-dir", str(evidence)],
                capture_output=True, check=False,
            )
            self.assertEqual(started.returncode, 0, started.stderr)
            start_record = json.loads(started.stdout)
            self.assertIn("review_session_sha256", start_record)
            time.sleep(0.01)
            finished = subprocess.run(
                [sys.executable, str(ROOT / "scripts/opencode-agent-task-pilot.py"),
                 "finish-review", "--evidence-dir", str(evidence), "--reviewer-id", "r1",
                 "--verdict", "reject", "--blinded"],
                capture_output=True, check=False,
            )
            self.assertEqual(finished.returncode, 0, finished.stderr)
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["assurance"], "timed_packet_bound")
            self.assertGreaterEqual(result["review_wall_ms"], 1)

    def test_intervene_and_record_review_cli_subcommands(self):
        with tempfile.TemporaryDirectory(prefix="spx-cli-eligibility-") as temp:
            evidence = Path(temp)
            elig.initialize_intervention_ledger(evidence)
            (evidence / "candidate.diff").write_text("--- a\n+++ b\n")
            packet = prepare_review_packet(evidence, evidence / "review-packet.json")
            intervene = subprocess.run(
                [sys.executable, str(ROOT / "scripts/opencode-agent-task-pilot.py"),
                 "intervene", "--evidence-dir", str(evidence),
                 "--kind", "manual_process_kill", "--target", "pid-42", "--note", "hung agent"],
                capture_output=True, check=False,
            )
            self.assertEqual(intervene.returncode, 0, intervene.stderr)
            self.assertEqual(json.loads(intervene.stdout)["sequence"], 1)

            review = subprocess.run(
                [sys.executable, str(ROOT / "scripts/opencode-agent-task-pilot.py"),
                 "record-review", "--evidence-dir", str(evidence), "--reviewer-id", "r1",
                 "--started-monotonic-ns", "0", "--stopped-monotonic-ns", "600000000000",
                 "--active-ms", "400000", "--blinded", "--packet", str(evidence / "review-packet.json"),
                 "--candidate-digest", packet["candidate_digest"], "--verdict", "accept"],
                capture_output=True, check=False,
            )
            self.assertEqual(review.returncode, 0, review.stderr)
            self.assertEqual(json.loads(review.stdout)["reviewer_id"], "r1")

            ledger = elig.intervention_ledger(evidence)
            self.assertEqual(ledger["human_interventions"], 1)
            self.assertEqual(elig.blinded_review_slot(evidence)["status"], "observed")

    def test_record_review_without_blinded_flag_is_rejected_by_cli(self):
        with tempfile.TemporaryDirectory(prefix="spx-cli-eligibility-") as temp:
            evidence = Path(temp)
            (evidence / "candidate.diff").write_text("--- a\n+++ b\n")
            review = subprocess.run(
                [sys.executable, str(ROOT / "scripts/opencode-agent-task-pilot.py"),
                 "record-review", "--evidence-dir", str(evidence), "--reviewer-id", "r1",
                 "--started-monotonic-ns", "0", "--stopped-monotonic-ns", "1000000000",
                 "--active-ms", "400000"],
                capture_output=True, check=False,
            )
            self.assertNotEqual(review.returncode, 0)
            self.assertIn(b"--blinded", review.stderr)

    def test_review_packet_withholds_identity_and_binds_submission(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-packet-") as temp:
            evidence = Path(temp) / "evidence"
            evidence.mkdir()
            # Use committed pilot fixture bytes: the packet test is not a
            # synthetic label-only check.
            (evidence / "candidate.diff").write_bytes(
                (ROOT / "benchmarks/agent-task-comparison-v1/fixture/src/core.spx").read_bytes()
            )
            (evidence / "candidate-source.json").write_bytes(
                (ROOT / "benchmarks/agent-task-comparison-v1/fixture/semaprax.toml").read_bytes()
            )
            packet_path = evidence / "review-packet.json"
            packet = prepare_review_packet(evidence, packet_path)
            packet_text = packet_path.read_text()
            for identity in ("semaprax-source-first", "semaprax-graph-operational", pilot.MODEL,
                              "signature-migration-v1"):
                self.assertNotIn(identity, packet_text)
            digest = packet["candidate_digest"]
            review = elig.record_blinded_review(
                evidence, "reviewer", 0, 2_000_000_000, 1000, True,
                packet_path, digest, "accept",
            )
            self.assertEqual(review["candidate_digest"], digest)
            with self.assertRaises(ValueError):
                elig.record_blinded_review(
                    evidence, "reviewer-2", 0, 2_000_000_000, 1000, True,
                    packet_path, "0" * 64, "accept",
                )

    def test_review_packet_rejects_stale_candidate(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-stale-") as temp:
            evidence = Path(temp) / "evidence"
            evidence.mkdir()
            (evidence / "candidate.diff").write_text("old\n")
            packet_path = evidence / "review-packet.json"
            packet = prepare_review_packet(evidence, packet_path)
            (evidence / "candidate.diff").write_text("new\n")
            with self.assertRaises(ValueError):
                elig.record_blinded_review(evidence, "reviewer", 0, 2_000_000_000, 1000, True,
                                           packet_path, packet["candidate_digest"], "reject")

    def test_every_non_diff_evidence_mutation_invalidates_packet_review(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-members-") as temp:
            evidence = Path(temp) / "evidence"; evidence.mkdir()
            (evidence / "candidate.diff").write_text("diff\n")
            (evidence / "candidate-source.json").write_text("source\n")
            packet = prepare_review_packet(evidence, evidence / "review-packet.json")
            (evidence / "candidate-source.json").write_text("changed\n")
            with self.assertRaises(ValueError):
                elig.record_blinded_review(evidence, "r", 0, 2_000_000_000, 1, True,
                                           evidence / "review-packet.json", packet["candidate_digest"])

    def test_packet_deletion_replacement_and_corrupt_v2_are_unavailable(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-hostile-") as temp:
            evidence = Path(temp) / "evidence"; evidence.mkdir()
            (evidence / "candidate.diff").write_text("diff\n")
            packet = prepare_review_packet(evidence, evidence / "review-packet.json")
            elig.record_blinded_review(evidence, "r", 0, 2_000_000_000, 1, True,
                                       evidence / "review-packet.json", packet["candidate_digest"], "reject")
            (evidence / "review-packet.json").unlink()
            self.assertEqual(elig.blinded_review_slot(evidence)["status"], "unavailable")
            (evidence / "review-packet.json").write_text("{}\n")
            self.assertEqual(elig.blinded_review_slot(evidence)["status"], "unavailable")
            review = json.loads((evidence / "review.json").read_text())
            review["candidate_digest"] = "g" * 64
            (evidence / "review.json").write_text(json.dumps(review) + "\n")
            self.assertEqual(elig.blinded_review_slot(evidence)["status"], "unavailable")

    def test_embedded_diff_entry_tamper_and_invalid_lengths_fail_closed(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-tamper-") as temp:
            evidence = Path(temp) / "evidence"; evidence.mkdir()
            (evidence / "candidate.diff").write_text("diff\n")
            prepare_review_packet(evidence, evidence / "review-packet.json")
            packet = json.loads((evidence / "review-packet.json").read_text())
            packet["evidence"][0]["bytes"] += 1
            (evidence / "review-packet.json").write_text(json.dumps(packet, sort_keys=True, separators=(",", ":")) + "\n")
            with self.assertRaises(ValueError):
                workflow.load_review_packet(evidence / "review-packet.json")
            packet["evidence"][0]["bytes"] = False
            (evidence / "review-packet.json").write_text(json.dumps(packet, sort_keys=True, separators=(",", ":")) + "\n")
            with self.assertRaises(ValueError):
                workflow.load_review_packet(evidence / "review-packet.json")

    def test_packet_short_write_removes_partial_target_and_allows_retry(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-write-") as temp:
            evidence = Path(temp) / "evidence"; evidence.mkdir()
            (evidence / "candidate.diff").write_text("diff\n")
            real_write = workflow.os.write
            calls = {"count": 0}
            def fail_once(fd, body):
                calls["count"] += 1
                if calls["count"] == 2:
                    raise OSError("injected")
                return real_write(fd, body[:1])
            with mock.patch.object(workflow.os, "write", side_effect=fail_once):
                with self.assertRaises(ValueError):
                    prepare_review_packet(evidence, evidence / "review-packet.json")
            self.assertFalse((evidence / "review-packet.json").exists())
            prepare_review_packet(evidence, evidence / "review-packet.json")

    def test_ledger_short_writes_are_completed(self):
        with tempfile.TemporaryDirectory(prefix="spx-ledger-write-") as temp:
            elig.initialize_intervention_ledger(temp)
            real_write = elig.os.write
            with mock.patch.object(elig.os, "write", side_effect=lambda fd, body: real_write(fd, body[:1])):
                elig.append_intervention(temp, "other_operator_action", "target", timestamp_ns=0)
            self.assertEqual(elig.intervention_ledger(temp)["status"], "observed")

    def test_legacy_review_is_readable_but_not_eligible(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-legacy-") as temp:
            evidence = Path(temp); (evidence / "candidate.diff").write_text("diff\n")
            elig.record_blinded_review(evidence, "r", 0, 2_000_000_000, 1, True)
            result = elig.blinded_review_slot(evidence)
            self.assertEqual(result["assurance"], "legacy_unbound")
            self.assertFalse(elig.compute_eligibility(
                prompt="p",
                gateway_log_bytes=b"", drift_declared=False, evidence_dir=evidence)["eligible"])

    def test_symlink_and_oversize_evidence_are_rejected(self):
        with tempfile.TemporaryDirectory(prefix="spx-review-files-") as temp:
            evidence = Path(temp) / "evidence"; evidence.mkdir()
            outside = Path(temp) / "outside"; outside.write_text("diff\n")
            (evidence / "candidate.diff").symlink_to(outside)
            with self.assertRaises(ValueError):
                prepare_review_packet(evidence, evidence / "review-packet.json")
            (evidence / "candidate.diff").unlink()
            (evidence / "candidate.diff").write_bytes(b"x" * (2 * 1024 * 1024 + 1))
            with self.assertRaises(ValueError):
                prepare_review_packet(evidence, evidence / "review-packet.json")

    def test_cohort_audit_exact_accounting_and_failure_retention(self):
        with tempfile.TemporaryDirectory(prefix="spx-cohort-audit-") as temp:
            root = Path(temp) / "evidence"
            root.mkdir()
            manifest = Path(temp) / "manifest.json"
            manifest.write_text('{"tasks":[],"repetitions":1,"lanes":[]}')
            with self.assertRaises(ValueError):
                audit_cohort(root, manifest)

    def test_audit_cli_rejects_incomplete_cohort_nonzero(self):
        with tempfile.TemporaryDirectory(prefix="spx-cohort-cli-") as temp:
            result = subprocess.run([
                sys.executable, str(ROOT / "scripts/opencode-agent-task-pilot.py"),
                "audit-cohort", "--evidence-root", temp,
            ], capture_output=True, check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_review_and_audit_paths_do_not_open_network(self):
        with mock.patch("socket.socket", side_effect=AssertionError("network")) as opened:
            with tempfile.TemporaryDirectory() as temp:
                evidence = Path(temp); (evidence / "candidate.diff").write_text("diff\n")
                prepare_review_packet(evidence, evidence / "review-packet.json")
            self.assertFalse(opened.called)


class TwoModelProtocolTests(unittest.TestCase):
    def _protocol(self):
        return {
            "schema": workflow.PROTOCOL_SCHEMA,
            "id": "authorized-pilot-v1",
            "manifest_sha256": FROZEN_MANIFEST_SHA256,
            "runner_revision": "a" * 40,
            "budget_policy": "approved-capped-budget-v1",
            "egress_policy": "approved-egress-v1",
            "models": [
                {
                    "id": "model-a", "provider": "provider-a", "model": "model-a",
                    "revision": "2026-10-01", "configured_model": "provider-a/model-a",
                    "tokenizer": "tokenizer-a@1", "configuration": "temperature-0",
                },
                {
                    "id": "model-b", "provider": "provider-b", "model": "model-b",
                    "revision": "2026-10-02", "configured_model": "provider-b/model-b",
                    "tokenizer": "tokenizer-b@1", "configuration": "temperature-0",
                },
            ],
        }

    def _write_protocol(self, directory, protocol=None):
        path = Path(directory) / "protocol.json"
        path.write_bytes(json.dumps(protocol or self._protocol(), sort_keys=True, separators=(",", ":")).encode() + b"\n")
        return path

    def test_protocol_rejects_mutable_model_revision(self):
        with tempfile.TemporaryDirectory(prefix="spx-protocol-") as temp:
            protocol = self._protocol()
            protocol["models"][0]["revision"] = "latest"
            with self.assertRaisesRegex(ValueError, "mutable"):
                workflow.load_frozen_protocol(self._write_protocol(temp, protocol))

    def test_protocol_audit_requires_all_thirty_six_model_tuples(self):
        with tempfile.TemporaryDirectory(prefix="spx-protocol-audit-") as temp:
            root = Path(temp) / "evidence"; root.mkdir()
            protocol_path = self._write_protocol(temp)
            protocol, protocol_digest = workflow.load_frozen_protocol(protocol_path)
            tasks = sorted(FROZEN_TASK_SHA256)
            lanes = ("semaprax-graph-operational", "semaprax-source-first")
            for model in protocol["models"]:
                for task in tasks:
                    for lane in lanes:
                        for trial in range(1, 4):
                            evidence = root / f"{model['id']}-{task}-{lane}-{trial}"
                            evidence.mkdir()
                            record = {
                                "task": task, "lane": lane, "trial": trial,
                                "manifest_sha256": FROZEN_MANIFEST_SHA256,
                                "task_sha256": FROZEN_TASK_SHA256[task],
                                "fixture_inventory_sha256": FROZEN_FIXTURE_SHA256[task],
                                "status": "eligible", "outcome": "completed",
                                "protocol_sha256": protocol_digest, "model_identity": model,
                            }
                            (evidence / "record.json").write_text(json.dumps(record, sort_keys=True))
            audit = audit_cohort(root, ROOT / "benchmarks/agent-task-comparison-v1/manifest.json", protocol_path)
            self.assertEqual(audit["expected_tuples"], 36)
            self.assertTrue(audit["complete"])
            self.assertTrue(audit["eligible_for_scoring"])
            self.assertFalse(audit["historical_evidence"])
            (root / "model-b-owned-signature-migration-v1-semaprax-source-first-3" / "record.json").unlink()
            incomplete = audit_cohort(root, ROOT / "benchmarks/agent-task-comparison-v1/manifest.json", protocol_path)
            self.assertFalse(incomplete["complete"])
            self.assertFalse(incomplete["eligible_for_scoring"])

    def test_protocol_free_audit_is_explicitly_historical_and_ineligible(self):
        with tempfile.TemporaryDirectory(prefix="spx-historical-audit-") as temp:
            root = Path(temp) / "evidence"; root.mkdir()
            audit = audit_cohort(root, ROOT / "benchmarks/agent-task-comparison-v1/manifest.json")
            self.assertTrue(audit["historical_evidence"])
            self.assertFalse(audit["eligible_for_scoring"])
            self.assertEqual(audit["claims"]["historical_evidence"], "explicitly_ineligible")


if __name__ == "__main__":
    unittest.main()
