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
            "approval": "fixture only; not spend authority", "cli_version": "2.1.286",
            "controller_ledger": "/fixture/controller-ledger",
            "hosts": {
                "mac-fixture": {"native_platform": "darwin-arm64", "kernel_release": "fixture-darwin", "boot_id": None,
                                "executable": "/fixture/mac/claude", "claude_sha256": "0" * 64, "home": "/fixture/mac/home", "login": "fixture"},
                "second-fixture": {"native_platform": "linux-arm64", "kernel_release": "fixture-linux", "boot_id": "11111111-1111-1111-1111-111111111111",
                                   "executable": "/fixture/linux/claude", "claude_sha256": "1" * 64, "home": "/fixture/linux/home", "login": "fixture"}},
            "execution_profiles": {"darwin-arm64": {"profile": p.DARWIN_PROFILE, "provision_sha256": None},
                                   "linux-arm64": {"profile": p.LINUX_PROFILE, "provision_sha256": "2" * 64}}}


def envelope():
    return {"type": "result", "subtype": "success", "is_error": False, "num_turns": 1,
            "permission_denials": [], "stop_reason": "end_turn", "terminal_reason": "completed", "queued_turn_count": 0,
            "modelUsage": {"claude-haiku-4-5": {"provider": "firstParty", "canonicalModel": "claude-haiku-4-5"}},
            "subagent_stats": {"spawned": 0}, "total_cost_usd": 0.02,
            "usage": {"input_tokens": 100, "output_tokens": 30, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 20},
            "result": json.dumps({"validate.ts": "export function validate(): number { return 0; }\n"})}


def generation_fixture(plan):
    host = plan["configuration"]["hosts"]["mac-fixture"]
    model = plan["configuration"]["models"][0]
    wire = json.dumps(envelope()).encode()
    transport = {"dispatches": 1, "provider_host": {key: host[key] for key in ("native_platform", "kernel_release", "boot_id")},
                 "host_authority_sha256": p.digest(p.canonical(host)), "cli_sha256": host["claude_sha256"],
                 "cli_version": plan["configuration"]["cli_version"], "requested_model": model["requested_model"],
                 "prompt_sha256": plan["prompt_sha256"], "failure": None, "exit_code": 0,
                 "argv": c.model_arguments(plan, model), "stdout_base64": base64.b64encode(wire).decode(),
                 "cli_version_receipt": {"exit_code": 0, "failure": None,
                    "stdout_base64": base64.b64encode(b"2.1.286 (Claude Code)\n").decode()}}
    response = c.decode(wire, model, plan["configuration"]["limits"])
    response["transport_receipt"] = transport
    return {"schema": "benchmark.cross_language.live_pilot_trial.v1", "plan_sha256": p.digest(p.canonical(plan)),
            "invocation_id": "fixture-invocation", "host_id": "mac-fixture", "model_id": "a",
            "task_id": p.TASK, "adapter_id": "typescript", "status": "generated", "model_dispatches": 1,
            "score": None, "response": response, "reason": None,
            "candidate_files_sha256": p.digest(p.canonical(response["candidate_files"])),
            "source_admission": p.source_admission(plan, "local_git_checkout")}


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

    def test_guest_snapshot_requires_external_pin_and_exact_local_bytes_without_git(self):
        plan = p.freeze(configuration())
        expected = p.digest(p.canonical(plan))
        with mock.patch.object(p, "runner_revision", side_effect=AssertionError("guest must not execute Git")):
            self.assertEqual(p.admit_guest(plan, expected), plan)
            for field, value in (("runner_revision", "0" * 40), ("prompt", "forged prompt"),
                                 ("source_manifest_sha256", "0" * 64), ("implementation", {})):
                changed = copy.deepcopy(plan)
                changed[field] = value
                with self.assertRaisesRegex(ValueError, "guest_plan_digest"):
                    p.admit_guest(changed, expected)
                if field != "runner_revision":
                    with self.assertRaisesRegex(ValueError, "guest_source_snapshot"):
                        p.admit_guest(changed, p.digest(p.canonical(changed)))
            with mock.patch.object(p, "implementation_identity", return_value={}):
                with self.assertRaisesRegex(ValueError, "guest_source_snapshot"):
                    p.admit_guest(plan, expected)
            with tempfile.TemporaryDirectory() as temporary:
                root = pathlib.Path(temporary).resolve()
                with mock.patch.object(runner, "ClaudeSubscription", side_effect=ValueError("fixture_no_dispatch")) as transport:
                    with self.assertRaisesRegex(ValueError, "guest_plan_digest"):
                        runner.generate_cell(plan, "a", "second-fixture", root / "bad", guest_plan_sha256="0" * 64)
                    with self.assertRaisesRegex(ValueError, "requires_linux"):
                        runner.generate_cell(plan, "a", "mac-fixture", root / "mac", guest_plan_sha256=expected)
                    transport.assert_not_called()
                    result = runner.generate_cell(plan, "a", "second-fixture", root / "linux", guest_plan_sha256=expected)
                    self.assertEqual(result["source_admission"], p.source_admission(plan, "controller_frozen_snapshot"))
                    self.assertEqual(result["model_dispatches"], 0)
                    self.assertEqual(result["status"], "failed")
                    transport.assert_called_once()
        # The controller retains ordinary Git-based admission and rejects a
        # snapshot label moved to the wrong host or a substituted source claim.
        receipt = generation_fixture(plan)
        for field, value in (("kind", "controller_frozen_snapshot"), ("controller_runner_revision", "0" * 40),
                             ("prompt_sha256", "0" * 64)):
            changed = copy.deepcopy(receipt)
            changed["source_admission"][field] = value
            with self.assertRaisesRegex(ValueError, "source_admission"):
                runner.admit_generation(plan, changed, p.digest(p.canonical(changed)))

    def test_sonnet_55_exact_version_pin_is_not_a_family_alias(self):
        config = configuration()
        config["models"][1].update(requested_model="claude-sonnet-5-5", reported_model="claude-sonnet-5-5")
        plan = p.freeze(config)
        self.assertEqual(plan["configuration"]["models"][1]["requested_model"], "claude-sonnet-5-5")
        for value in ("sonnet", "claude-sonnet", "claude-sonnet-5", "claude-sonnet-5-6", "claude-sonnet-5-5-latest"):
            bad = copy.deepcopy(config)
            bad["models"][1]["requested_model"] = value
            with self.assertRaisesRegex(ValueError, "exact_claude_snapshot_required"):
                p.freeze(bad)
        same = copy.deepcopy(config)
        same["models"][1]["reported_model"] = same["models"][0]["reported_model"]
        with self.assertRaisesRegex(ValueError, "distinct_models_required"):
            p.freeze(same)

    def test_native_host_authority_and_cli_version_are_not_labels(self):
        plan = p.freeze(configuration())
        host = plan["configuration"]["hosts"]["second-fixture"]
        with mock.patch.object(c, "native_identity", return_value={"native_platform": "darwin-arm64", "kernel_release": "fixture-darwin", "boot_id": None}), \
                mock.patch.object(c, "capture") as dispatch:
            with self.assertRaisesRegex(ValueError, "provider_native_host_mismatch"):
                c.ClaudeSubscription(host, pathlib.Path("/private/tmp"))
            dispatch.assert_not_called()
        for field, value in (("home", "relative"), ("executable", "/a/../claude"), ("claude_sha256", "missing"), ("boot_id", None)):
            config = configuration()
            config["hosts"]["second-fixture"][field] = value
            with self.assertRaises(ValueError):
                p.freeze(config)
        config = configuration()
        config["cli_version"] = "latest"
        with self.assertRaisesRegex(ValueError, "unreviewed_cli_version"):
            p.freeze(config)

    def test_generation_handoff_revalidates_provider_bytes_and_is_one_use(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            ledger = root / "ledger"
            ledger.mkdir(mode=0o700)
            config = configuration()
            config["controller_ledger"] = str(ledger)
            plan = p.freeze(config)
            generation = generation_fixture(plan)
            digest = p.digest(p.canonical(generation))
            runner.admit_generation(plan, generation, digest)
            for change in (
                lambda r: r["response"]["candidate_files"].update({p.CANDIDATE: "changed"}),
                lambda r: r["response"]["usage"].update(output_tokens=0),
                lambda r: r["response"]["transport_receipt"]["provider_host"].update(native_platform="linux-arm64"),
                lambda r: r["response"]["transport_receipt"].update(cli_sha256="f"*64),
                lambda r: r["response"]["transport_receipt"].update(argv=["different"]),
            ):
                bad = copy.deepcopy(generation)
                change(bad)
                with self.assertRaises(ValueError):
                    runner.admit_generation(plan, bad, p.digest(p.canonical(bad)))
            (root / "generation.json").write_bytes(p.canonical(generation))
            identity = {key: config["hosts"]["mac-fixture"][key] for key in ("native_platform", "kernel_release", "boot_id")}
            with mock.patch.object(runner, "native_identity", return_value=identity), \
                    mock.patch.object(runner, "CandidateSession") as factory, mock.patch.object(runner, "ClaudeSubscription") as provider:
                session = factory.return_value.__enter__.return_value
                # score_generation retains the context-manager object itself.
                session = factory.return_value
                session.score_candidate.return_value = {"status": "ok"}
                session.evidence.return_value = {"fixture": True}
                result = runner.score_generation(plan, root / "generation.json", digest, root)
                self.assertEqual(result["status"], "ok")
                provider.assert_not_called()
                self.assertEqual(session.score_candidate.call_count, 1)
                with self.assertRaises(FileExistsError):
                    runner.score_generation(plan, root / "generation.json", digest, root)
                # Changing an outer nonce cannot reuse a provider response for
                # the same planned cell under a new receipt digest.
                changed = copy.deepcopy(generation)
                changed["invocation_id"] = "different-fixture-invocation"
                (root / "generation2.json").write_bytes(p.canonical(changed))
                with self.assertRaises(FileExistsError):
                    runner.score_generation(plan, root / "generation2.json", p.digest(p.canonical(changed)), root)
                self.assertEqual(session.score_candidate.call_count, 1)

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
            host = plan["configuration"]["hosts"]["mac-fixture"]
            host.update(claude_sha256=p.digest(binary.read_bytes()), executable=str(binary), home=str(root))
            identity = {key: host[key] for key in ("native_platform", "kernel_release", "boot_id")}
            with mock.patch.object(c, "native_identity", return_value=identity):
                transport = c.ClaudeSubscription(host, root)
            receipt = {"exit_code": 0, "failure": None, "duration_ms": 1,
                       "stdout_base64": base64.b64encode(json.dumps(envelope()).encode()).decode(), "stderr_base64": ""}
            version = {"exit_code": 0, "failure": None, "duration_ms": 1,
                       "stdout_base64": base64.b64encode(b"2.1.286 (Claude Code)\n").decode(), "stderr_base64": ""}
            with mock.patch.object(c, "capture", side_effect=[version, receipt]) as capture:
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
                with mock.patch.object(c, "native_identity", return_value=identity):
                    c.ClaudeSubscription(host, root).complete(plan, plan["configuration"]["models"][0])

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
            _, _, _, public, hidden, _ = p.source_inputs()
            harness = public["index.ts"].decode().replace('import { validate } from "./validate";', 'const {validate}=require("./validate");').replace(
                'function assertEqual(actual: number, expected: number, label: string): void {',
                'function assertEqual(actual, expected, label) {')
            self.assertNotIn("console.", harness)
            cases = [
                ("exports.validate=(k,v,n)=>k!==7?1:v!==1?2:n<1||n>64?3:0;", True),
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
                self.assertEqual(result.stdout == ("\n" + COMPLETION + "\n").encode(), expected)

    def test_unadmitted_host_zero_dispatch_and_interrupted_inventory(self):
        plan = p.freeze(configuration())
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            with mock.patch.object(runner, "ClaudeSubscription", side_effect=ValueError("provider_native_host_mismatch")) as transport:
                receipt = runner.generate_cell(plan, "a", "mac-fixture", root / "trial")
                transport.assert_called_once()
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
