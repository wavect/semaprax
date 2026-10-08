import json
import hashlib
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import codex_campaign as campaign


ROOT = Path(__file__).resolve().parents[2]


class WebappCampaignTests(unittest.TestCase):
    def test_plan_pins_public_seed_receipt_and_matched_order(self):
        args = type("Args", (), {
            "repo": str(ROOT), "base_ref": "HEAD", "compiler_source_ref": "HEAD",
            "artifacts": "/private/tmp/teamdesk-offline-plan-test", "semaprax_bin": "/bin/sh",
            "tokenizer_dir": "/private/tmp/semaprax-opt-tokenizer", "codex_binary": "/usr/bin/true",
            "node_binary": "node", "playwright_root": str(ROOT / "benchmarks/webapp-tokens-v2/acceptance"),
            "model": campaign.MODEL, "effort": campaign.EFFORT, "trials_per_arm": 5,
            "timeout_seconds": campaign.TIMEOUT_SECONDS,
        })
        settings = campaign.plan(args)
        self.assertEqual(settings["attempt_denominator"], 10)
        self.assertEqual(settings["trial_order"], ["semaprax", "typescript", "typescript", "semaprax",
                                                      "semaprax", "typescript", "typescript", "semaprax",
                                                      "semaprax", "typescript"])
        self.assertEqual(settings["qualification"]["required_cases"], 912)
        self.assertEqual(set(settings["seed_files_sha256"]), {
            "benchmarks/webapp-tokens-v2/SPEC.md",
            "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md",
        })

    def test_seed_hashes_and_repository_are_exported_from_base_ref(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, seed = root / "repo", root / "seed"
            (repo / "benchmarks/webapp-tokens-v2/acceptance").mkdir(parents=True)
            subprocess.run(["git", "init", "--quiet", "--template="], cwd=repo, check=True)
            spec = repo / campaign.FROZEN_SPEC
            contract = repo / "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md"
            spec.write_text("spec-v1\n"); contract.write_text("contract-v1\n")
            subprocess.run(["git", "add", "."], cwd=repo, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "--quiet", "-m", "v1"], cwd=repo, check=True)
            first = subprocess.run(["git", "rev-parse", "HEAD"], cwd=repo, text=True,
                                   capture_output=True, check=True).stdout.strip()
            spec.write_text("spec-v2\n"); contract.write_text("contract-v2\n")
            subprocess.run(["git", "add", "."], cwd=repo, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "--quiet", "-m", "v2"], cwd=repo, check=True)
            hashes = campaign.pinned_seed_hashes(repo, first)
            self.assertEqual(hashes[campaign.FROZEN_SPEC], hashlib.sha256(b"spec-v1\n").hexdigest())
            self.assertNotEqual(hashes[campaign.FROZEN_SPEC], campaign.common.digest(spec))
            created = campaign.create_seed_repository(repo, first, seed)
            self.assertEqual(created["source_files_sha256"], hashes)
            self.assertEqual((seed / campaign.FROZEN_SPEC).read_bytes(), b"spec-v1\n")

    def test_harness_snapshot_contains_complete_acceptance_runtime_closure(self):
        commit = campaign.resolve_commit(ROOT, "HEAD")
        seed_hashes = campaign.pinned_seed_hashes(ROOT, commit)
        inventory = campaign.harness_source_inventory(ROOT, seed_hashes[campaign.FROZEN_SPEC])
        imports = set()
        for relative in campaign.ACCEPTANCE_SOURCE_FILES:
            if not relative.endswith(".mjs"):
                continue
            for line in (ROOT / relative).read_text().splitlines():
                if " from './" in line:
                    imported = line.split(" from './", 1)[1].split("'", 1)[0]
                    imports.add(f"benchmarks/webapp-tokens-v2/acceptance/{imported}")
        self.assertTrue(imports.issubset(inventory))
        self.assertIn("benchmarks/webapp-tokens-v2/acceptance/kdf-observer.mjs", inventory)
        self.assertIn(campaign.QUALIFICATION_RECEIPT, inventory)
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory) / "artifacts"; artifacts.mkdir()
            snapshot = campaign.snapshot_harness_sources(
                ROOT, artifacts, inventory, commit, seed_hashes[campaign.FROZEN_SPEC])
            copied = {path.relative_to(artifacts / "harness-source").as_posix()
                      for path in (artifacts / "harness-source").rglob("*") if path.is_file()}
            self.assertEqual(copied, {*campaign.HARNESS_SOURCE_FILES, "manifest.json"})
            self.assertEqual(snapshot["files_sha256"], inventory)

    def test_seed_contains_only_spec_and_launch_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            seed = Path(directory) / "seed"
            source_commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                           text=True, capture_output=True, check=True).stdout.strip()
            info = campaign.create_seed_repository(ROOT, source_commit, seed)
            self.assertEqual(info["seed_files_sha256"], info["source_files_sha256"])
            listed = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD"], cwd=seed,
                                    text=True, capture_output=True, check=True).stdout.splitlines()
            self.assertEqual(listed, [path.lstrip("/") for path in campaign.SEED_FILES])
            self.assertNotIn("benchmarks/webapp-tokens-v2/acceptance/run.mjs", listed)

    def test_rollout_usage_reconciles_only_matching_final_total(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stream = root / "exec.jsonl"
            rollout = root / "rollout.jsonl"
            usage = {"input_tokens": 20, "cached_input_tokens": 4,
                     "cache_write_input_tokens": 8, "output_tokens": 3,
                     "reasoning_output_tokens": 1}
            stream.write_text(json.dumps({"type": "turn.completed", "usage": usage}) + "\n")
            rollout.write_text("\n".join([
                json.dumps({"type": "turn_context", "payload": {"model": campaign.MODEL, "effort": campaign.EFFORT}}),
                json.dumps({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage}}),
            ]) + "\n")
            observed = campaign.trace_usage(campaign.parse_exec_jsonl(stream), rollout)
            self.assertTrue(observed["reconciled"])
            self.assertEqual(observed["model_request_count"], 1)
            self.assertEqual(campaign.list_price_estimate(observed["model_requests"])["actual_billed_usd"], None)

    def test_paid_acceptance_timeout_returns_a_persistable_failed_row(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root / "artifacts"; artifacts.mkdir()
            seed = root / "seed"; seed.mkdir()
            spec_bytes, contract_bytes = b"spec\n", b"contract\n"
            settings = {
                "codex_binary": "codex", "timeout_seconds": 1800,
                "acceptance_timeout_seconds": 2700, "authored_source_tokenizer": None,
                "seed_files_sha256": {
                    campaign.FROZEN_SPEC: hashlib.sha256(spec_bytes).hexdigest(),
                    "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md": hashlib.sha256(contract_bytes).hexdigest(),
                },
            }

            def add_worktree(_repo, workspace, _commit):
                (workspace / "benchmarks/webapp-tokens-v2/acceptance").mkdir(parents=True)
                (workspace / campaign.FROZEN_SPEC).write_bytes(spec_bytes)
                (workspace / "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md").write_bytes(contract_bytes)
                return None

            def paid_run(_command, workspace, _env, stream, stderr, _timeout):
                self.assertFalse((workspace / "candidate").exists())
                (workspace / "candidate").mkdir()
                (workspace / "candidate/app.ts").write_text("export {};\n")
                stream.write_text("\n"); stderr.write_text("")
                return {"process_exit_code": 0, "timed_out": False, "elapsed_seconds": 1.25}

            trace = {"reconciled": True, "model_observed": campaign.MODEL,
                     "effort_observed": campaign.EFFORT, "model_requests": [],
                     "request_usage_sum": {}, "legacy_net_input_tokens": 0}
            (root / "trace.jsonl").write_text("{}\n")
            parsed = {"thread_ids": ["00000000-0000-0000-0000-000000000000"],
                      "invalid_stream_lines": 0, "tool_item_counts": {}}
            with patch.object(campaign, "add_seed_worktree", side_effect=add_worktree), \
                    patch.object(campaign, "run_codex", side_effect=paid_run), \
                    patch.object(campaign, "parse_exec_jsonl", return_value=parsed), \
                    patch.object(campaign, "copy_task_rollout", return_value=root / "trace.jsonl"), \
                    patch.object(campaign, "trace_usage", return_value=trace), \
                    patch.object(campaign, "check_candidate",
                                 side_effect=subprocess.TimeoutExpired(["node", "run.mjs"], 2700)), \
                    patch.object(campaign, "cleanup_trial"):
                row = campaign.launch_trial(seed, artifacts, "seed-commit",
                                            {"arm": "typescript", "number": 1}, settings, root / "semaprax")
            self.assertEqual(row["status"], "failed")
            self.assertTrue(row["runner_error"])
            self.assertIn("timed out", row["failure"])
            self.assertEqual(row["elapsed_seconds"], 1.25)
            self.assertEqual(row["candidate_files_sha256"], {"app.ts": campaign.common.digest(
                Path(row["candidate_archive"]) / "app.ts")})

    def test_workspace_guard_refuses_seed_and_outside_candidate_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            spec = workspace / campaign.FROZEN_SPEC
            contract = workspace / "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md"
            contract.parent.mkdir(parents=True); spec.write_text("spec\n"); contract.write_text("contract\n")
            (workspace / "candidate").mkdir(); (workspace / "candidate/app.ts").write_text("ok\n")
            settings = {"seed_files_sha256": {
                campaign.FROZEN_SPEC: campaign.common.digest(spec),
                "benchmarks/webapp-tokens-v2/acceptance/CONTRACT.md": campaign.common.digest(contract),
            }}
            self.assertEqual(campaign.workspace_guard(workspace, settings)["status"], "passed")
            self.assertEqual(campaign.workspace_guard(workspace, settings, allow_candidate=False)["status"], "failed")
            (workspace / "outside.txt").write_text("escape\n")
            self.assertEqual(campaign.workspace_guard(workspace, settings)["status"], "failed")
            (workspace / "outside.txt").unlink(); spec.write_text("changed\n")
            self.assertEqual(campaign.workspace_guard(workspace, settings)["status"], "failed")

    def test_missing_failed_candidate_archives_as_an_empty_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hashes, omitted = campaign.archive_candidate(root / "missing-candidate", root / "archive")
            self.assertEqual((hashes, omitted), ({}, []))
            self.assertEqual(list((root / "archive").iterdir()), [])

    def test_candidate_check_runs_one_snapshotted_gate_and_requires_exact_report_schema(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); candidate = root / "candidate"; candidate.mkdir()
            gate_relative = "benchmarks/webapp-tokens-v2/acceptance/run.mjs"
            gate = root / "artifacts/harness-source" / gate_relative
            gate.parent.mkdir(parents=True); gate.write_text("// frozen gate\n")
            gate_hash = campaign.common.digest(gate)
            settings = {
                "artifacts": str(root / "artifacts"),
                "harness_source_snapshot": {"path": "harness-source", "files_sha256": {gate_relative: gate_hash}},
                "acceptance": {"runner": gate_relative, "capabilities": {
                    "node_binary": "/pinned/node", "playwright_root": "/pinned/playwright",
                }},
                "qualification": {"spec_sha256": "a" * 64},
                "compiler_source_commit": "b" * 40, "source_binary_sha256": "c" * 64,
                "acceptance_timeout_seconds": 2700,
            }

            def run_gate(command, **kwargs):
                self.assertEqual(command[0], "/pinned/node")
                self.assertNotIn("/bin/sh", command)
                output = Path(command[command.index("--output") + 1]); output.mkdir(parents=True)
                report = {
                    "schema": "semaprax.teamdesk.acceptance.v1", "arm": "typescript",
                    "spec_sha256": "a" * 64,
                    "gate": [{"name": "run.mjs", "sha256": gate_hash}],
                    "checks": [{"id": f"case-{index}", "status": "passed"} for index in range(912)],
                    "qualification": {"passed": True, "cases": 912, "missingCases": [],
                                      "missingGroups": [], "failures": []},
                    "candidate_after": [],
                }
                (output / "report.json").write_text(json.dumps(report))
                return subprocess.CompletedProcess(command, 0, "ok", "")

            with patch.object(campaign.subprocess, "run", side_effect=run_gate) as invoked:
                accepted = campaign.check_candidate(candidate, root / "evidence-good", "typescript",
                                                    settings, root / "semaprax")
            self.assertTrue(accepted["accepted"])
            self.assertEqual(invoked.call_count, 1)

            def run_bad_gate(command, **kwargs):
                result = run_gate(command, **kwargs)
                output = Path(command[command.index("--output") + 1])
                report = json.loads((output / "report.json").read_text())
                report["schema"] = "wrong"
                (output / "report.json").write_text(json.dumps(report))
                return result

            with patch.object(campaign.subprocess, "run", side_effect=run_bad_gate):
                rejected = campaign.check_candidate(candidate, root / "evidence-bad", "typescript",
                                                    settings, root / "semaprax")
            self.assertFalse(rejected["accepted"])
            self.assertIn("report schema", rejected["failure"])

    def test_snapshotted_node_gate_receives_launch_paths_and_writes_real_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); artifacts = root / "artifacts"; artifacts.mkdir()
            commit = campaign.resolve_commit(ROOT, "HEAD")
            seeds = campaign.pinned_seed_hashes(ROOT, commit)
            inventory = campaign.harness_source_inventory(ROOT, seeds[campaign.FROZEN_SPEC])
            snapshot = campaign.snapshot_harness_sources(
                ROOT, artifacts, inventory, commit, seeds[campaign.FROZEN_SPEC])
            candidate = root / "candidate"; candidate.mkdir()
            (candidate / "build.sh").write_text("printf 'build\\n' >> \"$TEAMDESK_DATA_DIR/scripts.log\"\n")
            (candidate / "test.sh").write_text("printf 'test\\n' >> \"$TEAMDESK_DATA_DIR/scripts.log\"\n")
            (candidate / "run.sh").write_text(
                "printf '%s\\n' \"$TEAMDESK_ARM|$TEAMDESK_HOST|$TEAMDESK_PORT|$TEAMDESK_UI_PORT|$TEAMDESK_DATA_DIR\" "
                "> \"$TEAMDESK_DATA_DIR/launch-env.txt\"\nexit 1\n")
            node = shutil.which("node")
            self.assertIsNotNone(node)
            settings = {
                "artifacts": str(artifacts), "harness_source_snapshot": snapshot,
                "acceptance": {"runner": "benchmarks/webapp-tokens-v2/acceptance/run.mjs",
                               "capabilities": {"node_binary": node,
                                                "playwright_root": str(ROOT / "benchmarks/webapp-tokens-v2/acceptance")}},
                "qualification": {"spec_sha256": seeds[campaign.FROZEN_SPEC]},
                "compiler_source_commit": commit, "source_binary_sha256": "unused",
                "acceptance_timeout_seconds": 30,
            }
            output = root / "evidence"
            result = campaign.check_candidate(candidate, output, "typescript", settings, root / "semaprax")
            self.assertFalse(result["accepted"])
            report = result["report"]
            self.assertEqual(report["schema"], "semaprax.teamdesk.acceptance.v1")
            self.assertEqual(report["arm"], "typescript")
            self.assertEqual(report["spec_sha256"], seeds[campaign.FROZEN_SPEC])
            expected_gate = {name.removeprefix("benchmarks/webapp-tokens-v2/acceptance/"): digest
                             for name, digest in inventory.items()
                             if name.startswith("benchmarks/webapp-tokens-v2/acceptance/")}
            self.assertEqual({row["name"]: row["sha256"] for row in report["gate"]}, expected_gate)
            self.assertEqual((output / "data/scripts.log").read_text().splitlines(), ["build", "test"])
            arm, host, api_port, ui_port, data = (output / "data/launch-env.txt").read_text().strip().split("|")
            self.assertEqual((arm, host), ("typescript", "127.0.0.1"))
            self.assertTrue(api_port.isdecimal() and ui_port.isdecimal())
            self.assertEqual(Path(data), output / "data")

    def test_summary_keeps_usage_cost_and_wall_time_separate_from_calibration(self):
        def row(status, raw, net, authored, price, agent, acceptance):
            return {"status": status, "observed": {"request_usage_sum": {
                        "input_tokens": raw, "cached_input_tokens": 2,
                        "cache_write_input_tokens": 3, "output_tokens": 4,
                    }, "legacy_net_input_tokens": net},
                    "final_candidate_source_metrics": {"total_tokens": authored},
                    "list_price": {"standard_short_context_api_equivalent_usd": price},
                    "elapsed_seconds": agent, "acceptance": {"seconds": acceptance}}
        summary = campaign.summarize([
            row("accepted", 100, 40, 800, 1.25, 10, 20),
            row("not_accepted", 200, 60, 900, 2.75, 30, 40),
        ])
        self.assertEqual(summary["usage_all_attempts"]["raw_input_tokens"], 300)
        self.assertEqual(summary["usage_all_attempts"]["legacy_net_input_tokens"], 100)
        self.assertEqual(summary["authored_source_tokens_per_accepted_task"], 800)
        self.assertEqual(summary["list_price_estimate_per_accepted_task_usd"], 4.0)
        self.assertEqual(summary["agent_wall_seconds_per_accepted_task"], 40)
        self.assertEqual(summary["acceptance_wall_seconds_per_accepted_task"], 60)
        self.assertTrue(summary["calibration_separate"])


if __name__ == "__main__":
    unittest.main()
