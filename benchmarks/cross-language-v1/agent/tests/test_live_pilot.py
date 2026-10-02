"""No provider calls: frozen inventory, candidate custody, process and wire gates."""
import base64
import copy
import json
import os
import pathlib
import sys
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

SUITE = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(SUITE))
from agent import pilot_protocol as p
from agent import claude_subscription as c
from agent import pilot_run as runner
from agent.pilot_score import CandidateSession, install_assertion_bridge, COMPLETION


def configuration():
    return {"models": [{"id": "a", "requested_model": "claude-haiku-4-5-20251001", "reported_model": "claude-haiku-4-5"},
                       {"id": "b", "requested_model": "claude-sonnet-4-5-20250929", "reported_model": "claude-sonnet-4-5"}],
            "limits": {"deadline_seconds": 90, "max_request_bytes": 65536, "max_result_bytes": 65536,
                       "max_reported_tokens": 16384, "max_estimated_usd": 1.0},
            "claude_sha256": "0" * 64, "approval": "fixture only; not spend authority",
            "host_ids": ["mac-fixture", "second-fixture"]}


def envelope():
    return {"type": "result", "subtype": "success", "is_error": False, "num_turns": 1,
            "permission_denials": [], "stop_reason": "end_turn", "terminal_reason": "completed", "queued_turn_count": 0,
            "modelUsage": {"claude-haiku-4-5": {"provider": "firstParty", "canonicalModel": "claude-haiku-4-5"}},
            "subagent_stats": {"spawned": 0}, "total_cost_usd": 0.02,
            "usage": {"input_tokens": 100, "output_tokens": 30, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 20},
            "result": json.dumps({"validate.ts": "export function validate(): number { return 0; }\n"})}


class PilotTests(unittest.TestCase):
    def test_frozen_inventory_prompt_and_identity(self):
        plan = p.freeze(configuration())
        self.assertEqual(len(plan["comparison_inventory"]), 182)
        self.assertEqual(len(plan["trials_per_host"]), 28)
        self.assertEqual(sum(row["disposition"] == "planned" for row in plan["trials_per_host"]), 2)
        self.assertEqual(plan["sampling"]["seed"], None)
        _, _, _, public, hidden, prompt = p.source_inputs()
        self.assertNotIn(public[p.CANDIDATE].decode(), prompt)
        self.assertNotIn(hidden["index.ts"].decode(), prompt)
        self.assertIn(public["index.ts"].decode(), prompt)
        self.assertEqual(p.admit(plan, p.digest(p.canonical(plan))), plan)
        changed = copy.deepcopy(plan)
        changed["trials_per_host"].pop()
        with self.assertRaisesRegex(ValueError, "frozen_pilot_binding"):
            p.admit(changed, p.digest(p.canonical(changed)))
        for field, value in (("requested_model", "claude-haiku-4-5"), ("id", "../escape")):
            config = configuration()
            config["models"][0][field] = value
            with self.assertRaises(ValueError):
                p.freeze(config)

    def test_candidate_hidden_collision_and_extra_file_refuse(self):
        with self.assertRaisesRegex(ValueError, "candidate_hidden_overlay_collision"):
            p.admit_paths({"index.ts": b"candidate"}, {"index.ts": b"oracle"}, ("index.ts",))
        for path in ("../validate.ts", "/validate.ts", "x\\validate.ts", "x/../validate.ts", "index.ts"):
            with self.assertRaises(ValueError):
                p.admit_paths({"validate.ts": b""}, {"index.ts": b""}, (path,))

    def test_observed_usage_model_and_exact_candidate_transport(self):
        config = configuration()
        for fenced in (False, True):
            wire = envelope()
            if fenced:
                wire["result"] = "```json\n" + wire["result"] + "\n```"
            result = c.decode(json.dumps(wire), config["models"][0], config["limits"])
            self.assertTrue(result["candidate_files"]["validate.ts"].endswith("\n"))
            self.assertEqual(result["usage"]["cache_read_input_tokens"], 20)
            self.assertIsNone(result["subscription_invoice_cost_usd"])
            self.assertFalse(result["requested_snapshot_observed_directly"])
        for mutate in (
                lambda w: w.update(num_turns=2), lambda w: w.update(permission_denials=[{}]),
                lambda w: w.update(modelUsage={"other": {}}), lambda w: w.update(subagent_stats={"spawned": 1}),
                lambda w: w["usage"].update(output_tokens=True), lambda w: w["usage"].update(output_tokens=20000),
                lambda w: w.update(total_cost_usd=2), lambda w: w.update(result='{"index.ts":"fake"}'),
                lambda w: w.update(result='{"validate.ts":"a","validate.ts":"b"}'),
                lambda w: w.update(result='prose ' + w["result"]),
                lambda w: w.update(result='```typescript\n' + w["result"] + '\n```')):
            wire = envelope()
            mutate(wire)
            with self.assertRaises(ValueError):
                c.decode(json.dumps(wire), config["models"][0], config["limits"])

    def test_bounded_capture_drains_both_pipes_and_kills_descendants(self):
        with tempfile.TemporaryDirectory() as directory:
            result = c.capture([sys.executable, "-c", "import sys;sys.stderr.write('e'*50000);sys.stdout.write('o'*50000)"],
                               directory, {"PATH": "/usr/bin:/bin"}, b"request", 5)
            self.assertIsNone(result["failure"])
            self.assertEqual(len(base64.b64decode(result["stdout_base64"])), 50000)
            result = c.capture([sys.executable, "-c", "import sys;sys.stdout.write('x'*100000)"],
                               directory, {}, b"", 5, maximum=1024)
            self.assertEqual(result["failure"], "provider_output_bound")
            self.assertEqual(len(base64.b64decode(result["stdout_base64"])), 1024)
            result = c.capture([sys.executable, "-c", "import os,time;pid=os.fork();time.sleep(5) if pid==0 else None"],
                               directory, {}, b"", 0.15)
            self.assertEqual(result["failure"], "provider_deadline")
            self.assertLess(result["duration_ms"], 2000)

    def test_transport_is_single_use_closed_env_and_digest_bound(self):
        plan = p.freeze(configuration())
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp).resolve()
            binary = root / "claude"
            binary.write_bytes(b"fixture executable; never started")
            plan["configuration"]["claude_sha256"] = p.digest(binary.read_bytes())
            transport = c.ClaudeSubscription(binary, root, "fixture", root)
            receipt = {"exit_code": 0, "failure": None, "duration_ms": 1,
                       "stdout_base64": base64.b64encode(json.dumps(envelope()).encode()).decode(), "stderr_base64": ""}
            with mock.patch.object(c, "capture", return_value=receipt) as capture:
                response = transport.complete(plan, plan["configuration"]["models"][0])
                args = capture.call_args.args
                self.assertEqual(args[2]["USER"], "fixture")
                self.assertNotIn("ANTHROPIC_API_KEY", args[2])
                self.assertIn("--restricted", args[0])
                self.assertEqual(args[3], plan["prompt"].encode())
                self.assertEqual(response["transport_receipt"]["dispatches"], 1)
                with self.assertRaisesRegex(ValueError, "transport_already_consumed"):
                    transport.complete(plan, plan["configuration"]["models"][0])
            binary.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "executable_digest_mismatch"):
                c.ClaudeSubscription(binary, root, "fixture", root).complete(plan, plan["configuration"]["models"][0])

    def test_scoring_keeps_same_candidate_in_both_phases(self):
        plan = p.freeze(configuration())
        _, sources, task, public, hidden, _ = p.source_inputs()
        with tempfile.TemporaryDirectory() as temporary:
            session = CandidateSession.__new__(CandidateSession)
            session.root = pathlib.Path(temporary).resolve()
            session.authority = type("Commands", (), {"commands": []})()
            session.artifacts, session.results = [], []
            seen = []
            def stage(directory):
                seen.append((directory.name, (directory / p.CANDIDATE).read_bytes(), (directory / "index.ts").read_bytes()))
                return {"passed": False, "phase": "run", "detail": ["fixture wrong candidate"]}
            with mock.patch.object(session, "admit_pilot", return_value=(task, public, hidden)), \
                    mock.patch.object(session, "_stage", side_effect=stage), mock.patch.object(session, "_capture"):
                result = session.score_candidate(plan, {p.CANDIDATE: "export const validate = () => 99;\n"})
            self.assertEqual(result["status"], "failed")
            self.assertEqual([row[0] for row in seen], ["public", "hidden"])
            self.assertEqual(seen[0][1], seen[1][1])
            self.assertEqual(seen[1][2], hidden["index.ts"])
            self.assertNotEqual(seen[0][2], seen[1][2])

    def test_numeric_bridge_rejects_early_exit_wrong_result_and_codegen(self):
        node = shutil.which("node")
        self.assertIsNotNone(node, "a local Node is required for the assertion-isolation fixture")
        with tempfile.TemporaryDirectory() as temporary:
            directory = pathlib.Path(temporary).resolve()
            harness = "const {validate}=require('./validate');if(validate(7,1,1)!==0)throw new Error('wrong');console.log('ok');\n"
            cases = [
                ("exports.validate=()=>0;", True),
                ("globalThis.process.exit(0);exports.validate=()=>999;", False),
                ("exports.validate=()=>999;", False),
                ("exports.validate=()=>Function('return process')().exit(0);", False),
                ("exports.validate=()=>{while(true){}};", False),
                ("exports.validate=()=>({valueOf:()=>0});", False),
            ]
            for candidate, expected in cases:
                (directory / "validate.js").write_text(candidate)
                (directory / "index.js").write_text(harness)
                original, bridge, completed = install_assertion_bridge(directory)
                self.assertEqual(original, candidate.encode())
                result = subprocess.run([node, "index.js"], cwd=directory, capture_output=True, timeout=5,
                                        env={"PATH": "/usr/bin:/bin"}, check=False)
                self.assertEqual(result.returncode == 0, expected, result.stderr.decode())
                self.assertEqual(result.stdout == ("ok\n\n" + COMPLETION + "\n").encode(), expected)

    def test_unadmitted_host_zero_dispatch_and_interrupted_inventory(self):
        plan = p.freeze(configuration())
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            with mock.patch.object(runner, "CandidateSession", side_effect=ValueError("host_profile_refused")), \
                    mock.patch.object(runner, "ClaudeSubscription") as transport:
                receipt = runner.run_cell(plan, "a", "mac-fixture", root / "trial", root, root / "claude", root, "fixture")
                transport.assert_not_called()
            self.assertEqual(receipt["model_dispatches"], 0)
            self.assertEqual(receipt["status"], "failed")
            report = runner.account(plan, "mac-fixture", [root / "trial"])
            self.assertEqual(len(report["trials"]), 28)
            self.assertFalse(report["all_planned_trials_have_receipts"])
            (root / "trial/result.json").unlink()
            report = runner.account(plan, "mac-fixture", [root / "trial"])
            self.assertTrue(any(row["observation"]["status"] == "interrupted" for row in report["trials"]))
            with self.assertRaisesRegex(ValueError, "duplicate_or_unplanned"):
                runner.account(plan, "mac-fixture", [root / "trial", root / "trial"])
            with self.assertRaises(FileExistsError):
                runner.write_new(root / "trial/intent.json", {})


if __name__ == "__main__":
    unittest.main()
