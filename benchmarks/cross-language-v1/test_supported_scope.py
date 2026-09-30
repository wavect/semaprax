"""Offline #322 scope/provenance regressions, not official-runtime evidence."""
from __future__ import annotations

import contextlib
import copy
import io
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

SUITE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(SUITE))
import supported_scope as scope

p = scope.provenance
c = scope.corrections
ADAPTERS = ["semaprax", "semaprax-project", "rust", "typescript", "c", "python",
            "swift", "java", "zero", "ntnt", "aver", "vera", "hale", "moonbit"]
ORIGINAL_FIELDS = ("task_id", "adapter_id", "declared", "implemented", "blocked_reason")


@unittest.skipUnless(scope.acquisition_available(), "existing POSIX no-follow acquisition unavailable")
class SupportedScopeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest, cls.sources = p.source_snapshot()
        cls.result = scope.report()

    def test_exact_supported_lane_and_all_explicit_exclusions(self):
        self.assertEqual(self.result["supported_adapter_ids"], ["typescript"])
        self.assertEqual(self.result["not_supported_adapter_ids"],
                         [name for name in ADAPTERS if name != "typescript"])
        self.assertEqual(self.result["decision"]["newly_admitted_lanes"], [])
        self.assertEqual(set(scope.EXCLUSIONS), set(ADAPTERS) - {"typescript"})
        self.assertTrue(all(reason.strip() for reason in scope.EXCLUSIONS.values()))

    def test_all_182_original_rows_and_their_order_are_unchanged(self):
        rows = self.result["comparison_inventory"]
        self.assertEqual(len(rows), 182)
        self.assertEqual([{key: row[key] for key in ORIGINAL_FIELDS} for row in rows],
                         self.manifest["comparison_inventory"])
        self.assertEqual([(row["task_id"], row["adapter_id"]) for row in rows],
                         [(task, adapter) for task in self.manifest["task_ids"] for adapter in ADAPTERS])

    def test_counts_are_computed_by_the_actual_complete_partition(self):
        rows = self.result["comparison_inventory"]
        counts = self.result["counts"]
        self.assertEqual(counts, {"tasks": 13, "adapters": 14, "slots": 182,
                                  "supported_slots": 13, "not_supported_slots": 169})
        self.assertEqual(sum(row["official_support"] == "supported" for row in rows), 13)
        self.assertEqual(sum(row["official_support"] == "not_supported" for row in rows), 169)

    def test_each_supported_slot_has_corrected_profile_and_real_source_inventory(self):
        for row in self.result["comparison_inventory"]:
            if row["official_support"] == "supported":
                self.assertEqual(row["adapter_id"], "typescript")
                self.assertIs(row["declared"], True)
                self.assertIs(row["implemented"], True)
                self.assertIsNone(row["blocked_reason"])
                self.assertIsNone(row["support_reason"])
                self.assertEqual(row["official_profile"], "benchmark.cross_language.runnable_adapter.v3")
        self.assertEqual(self.result["source_manifest_sha256"], p.SOURCE_HASH)
        self.assertEqual(self.result["source_correction_sha256"], c.HASH)
        self.assertEqual(len(self.manifest["files"]), 298)

    def test_all_169_excluded_slots_have_explicit_reasons_and_no_official_profile(self):
        rows = [row for row in self.result["comparison_inventory"] if row["adapter_id"] != "typescript"]
        self.assertEqual(len(rows), 169)
        for row in rows:
            with self.subTest(task=row["task_id"], adapter=row["adapter_id"]):
                self.assertEqual(row["official_support"], "not_supported")
                self.assertIn(scope.EXCLUSIONS[row["adapter_id"]], row["support_reason"])
                self.assertIsNone(row["official_profile"])
                if not row["declared"]:
                    self.assertIn("No public/hidden port is declared", row["support_reason"])

    def test_original_unimplemented_reasons_are_retained_not_replaced_by_policy(self):
        rows = [row for row in self.result["comparison_inventory"] if not row["implemented"]]
        self.assertEqual(len(rows), 78)
        self.assertTrue(all(row["blocked_reason"] and row["support_reason"] for row in rows))
        original = {(row["task_id"], row["adapter_id"]): row for row in self.manifest["comparison_inventory"]}
        for row in rows:
            self.assertEqual(row["blocked_reason"], original[row["task_id"], row["adapter_id"]]["blocked_reason"])

    def test_implemented_does_not_imply_independently_supported(self):
        rows = [row for row in self.result["comparison_inventory"]
                if row["implemented"] and row["official_support"] == "not_supported"]
        self.assertEqual(len(rows), 91)
        self.assertEqual({row["adapter_id"] for row in rows},
                         {"semaprax", "semaprax-project", "rust", "c", "python", "swift", "java"})

    def test_exact_host_and_toolchain_versions_are_not_generalized(self):
        profile = self.result["supported_profile"]
        self.assertEqual(profile["node"], "22.12.0")
        self.assertEqual(profile["typescript"], "5.8.3")
        self.assertEqual(profile["host"], {"platform": "darwin-arm64", "version": "26.5.1", "build": "25F80"})
        self.assertEqual(profile["runtime_gate"], "test_runnable_adapter_v3.py")

    def test_report_is_not_an_execution_receipt_or_score(self):
        self.assertEqual(self.result["status"], "scope_inventory")
        self.assertEqual(self.result["execution"], "not_attempted")
        self.assertEqual(self.result["runtime_availability"], "not_probed")
        for key in ("results", "commands", "authority", "elapsed", "performance"):
            self.assertNotIn(key, self.result)
        self.assertNotIn("official_conformance_observed", p.canonical(self.result).decode())

    def test_canonical_output_is_deterministic_and_within_existing_result_bound(self):
        first, second = p.canonical(scope.report()), p.canonical(scope.report())
        self.assertEqual(first, second)
        self.assertEqual(p.canonical(json.loads(first)), first)
        self.assertLessEqual(len(first), scope.v1.MAX_RESULT_BYTES)

    def test_existing_result_bound_is_enforced_not_raised(self):
        with mock.patch.object(scope.v1, "MAX_RESULT_BYTES", 1):
            with self.assertRaisesRegex(p.Error, "supported_scope_result_exceeds_bound"):
                scope.report()

    def test_projection_refuses_dropped_duplicated_reordered_or_relabelled_rows(self):
        for change in ("drop", "duplicate", "reorder", "adapter", "reason", "flag"):
            manifest = copy.deepcopy(self.manifest)
            rows = manifest["comparison_inventory"]
            if change == "drop":
                rows.pop()
            elif change == "duplicate":
                rows[-1] = copy.deepcopy(rows[0])
            elif change == "reorder":
                rows.reverse()
            elif change == "adapter":
                rows[0]["adapter_id"] = "typescript"
            elif change == "reason":
                rows[8]["blocked_reason"] = None
            else:
                rows[0]["implemented"] = False
            with self.subTest(change=change), self.assertRaisesRegex(p.Error, "supported_scope_inventory_mismatch"):
                scope._project_inventory(manifest, self.sources)

    def test_projection_refuses_unknown_missing_or_duplicated_adapter_ids(self):
        for change in ("unknown", "missing", "duplicate"):
            sources = dict(self.sources)
            adapters = json.loads(sources[scope.PREFIX + "adapters.json"])
            if change == "unknown":
                adapters["adapters"][-1]["id"] = "lean"
            elif change == "missing":
                adapters["adapters"].pop()
            else:
                adapters["adapters"][-1] = copy.deepcopy(adapters["adapters"][0])
            sources[scope.PREFIX + "adapters.json"] = p.canonical(adapters)
            with self.subTest(change=change), self.assertRaisesRegex(p.Error, "supported_scope_denominator_mismatch"):
                scope._project_inventory(self.manifest, sources)

    def test_projection_refuses_changed_task_denominator(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["task_ids"] = manifest["task_ids"][:-1]
        with self.assertRaisesRegex(p.Error, "supported_scope_denominator_mismatch"):
            scope._project_inventory(manifest, self.sources)

    def test_source_scorer_inventory_or_oracle_drift_refuses_before_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            for name, data in self.sources.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            names = [scope.PREFIX + name for name in ("run.py", "tasks.json", "adapters.json",
                     "tasks/stale-edit-preservation-v1/EQUIVALENCE.md")]
            for name in names:
                path = root / name
                original = path.read_bytes()
                try:
                    path.write_bytes(original + b"\n")
                    with self.subTest(path=name), mock.patch.object(p, "ROOT", root):
                        with self.assertRaisesRegex(p.Error, "approved_source_input_drifted"):
                            scope.report()
                finally:
                    path.write_bytes(original)

    def test_caller_reminted_manifest_cannot_admit_an_excluded_lane(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["comparison_inventory"][0]["adapter_id"] = "typescript"
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary).resolve() / "manifest.json"
            path.write_bytes(p.canonical(manifest))
            self.assertNotEqual(p.digest(path.read_bytes()), p.SOURCE_HASH)
            with mock.patch.object(p, "SOURCE_MANIFEST", path):
                with self.assertRaisesRegex(p.Error, "approved_source_manifest_drifted"):
                    scope.report()

    def test_correction_tamper_and_recomputed_payload_hash_do_not_revert_stale_edit(self):
        correction = json.loads(c.PATH.read_bytes())
        correction["files"][0]["effective"] = copy.deepcopy(correction["files"][0]["base"])
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary).resolve() / "correction.json"
            path.write_bytes(p.canonical(correction))
            with mock.patch.object(c, "PATH", path):
                with self.assertRaisesRegex(p.Error, "approved_source_correction_drifted"):
                    scope.report()

    @unittest.skipUnless(hasattr(os, "O_NOFOLLOW") and hasattr(os, "symlink"), "no POSIX no-follow paths")
    def test_leaf_and_ancestor_link_substitution_refuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            held = root / "held"
            held.mkdir()
            actual = held / "source.json"
            actual.write_bytes(p.SOURCE_MANIFEST.read_bytes())
            leaf = root / "leaf.json"
            leaf.symlink_to(actual)
            ancestor = root / "ancestor"
            ancestor.symlink_to(held, target_is_directory=True)
            for path in (leaf, ancestor / "source.json"):
                with self.subTest(path=path), mock.patch.object(p, "SOURCE_MANIFEST", path):
                    with self.assertRaisesRegex(p.Error, "nofollow_acquisition_refused"):
                        scope.report()

    def test_oversized_manifest_refuses_under_existing_acquisition_bound(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary).resolve() / "source.json"
            path.write_bytes(b" " * (128 * 1024 + 1))
            with mock.patch.object(p, "SOURCE_MANIFEST", path):
                with self.assertRaisesRegex(p.Error, "regular_file_type_or_size_refused"):
                    scope.report()

    def test_report_never_probes_host_or_dispatches_a_process(self):
        with mock.patch.object(p, "host_identity", side_effect=AssertionError("unexpected host probe")), \
             mock.patch("subprocess.run", side_effect=AssertionError("unexpected process")), \
             mock.patch("subprocess.Popen", side_effect=AssertionError("unexpected process")), \
             mock.patch("socket.socket", side_effect=AssertionError("unexpected network")):
            self.assertEqual(scope.report(), self.result)

    def test_ambient_tools_home_and_loader_environment_cannot_expand_support(self):
        with mock.patch.dict(os.environ, {"PATH": "/untrusted/toolchain", "HOME": "/untrusted/home",
                                         "NODE_OPTIONS": "--require=/untrusted/preload.js",
                                         "PYTHONPATH": "/untrusted/python",
                                         "DYLD_INSERT_LIBRARIES": "/untrusted/loader"}, clear=True):
            self.assertEqual(scope.report(), self.result)

    def test_cli_emits_the_complete_canonical_inventory(self):
        result = subprocess.run([sys.executable, str(SUITE / "supported_scope.py")],
                                cwd=p.ROOT, capture_output=True, timeout=30, check=False)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(result.stderr, b"")
        self.assertEqual(result.stdout, p.canonical(self.result))

    def test_cli_refuses_subset_toolchain_and_caller_policy_arguments(self):
        for args in (["--language", "typescript"], ["--only", "sequence-digest-v1"],
                     ["--policy", "/untrusted/policy.json"], ["--toolchain", "/untrusted/toolchain"]):
            result = subprocess.run([sys.executable, str(SUITE / "supported_scope.py"), *args],
                                    cwd=p.ROOT, capture_output=True, timeout=30, check=False)
            with self.subTest(args=args):
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b"")
                self.assertIn(b"unrecognized arguments", result.stderr)

    def test_cli_failure_emits_no_partial_or_success_inventory(self):
        stdout, stderr = io.StringIO(), io.StringIO()
        with mock.patch.object(p, "source_snapshot", side_effect=p.Error("approved_source_input_drifted")), \
             contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            self.assertEqual(scope.main([]), 2)
        self.assertEqual(stdout.getvalue(), "")
        self.assertEqual(json.loads(stderr.getvalue()), {
            "schema": scope.SCHEMA, "status": "unavailable", "execution": "not_attempted",
            "reason": "approved_source_input_drifted"})

    def test_documented_supported_set_matches_the_complete_machine_inventory(self):
        doc = (p.ROOT / "docs/CROSS-LANGUAGE-RUNNABLE-ADAPTER-V3.md").read_text(encoding="utf-8")
        self.assertIn("## 9. Supported runnable set (issue #322)", doc)
        section = doc.split("## 9. Supported runnable set (issue #322)", 1)[1]
        rows = [line for line in section.splitlines() if line.startswith("| `")]
        self.assertEqual(len(rows), 14)
        for row, adapter in zip(rows, ADAPTERS):
            columns = [column.strip() for column in row.strip("|").split("|")]
            self.assertEqual(columns[0], f"`{adapter}`")
            self.assertEqual(columns[1], "Supported, fixed v3 profile" if adapter == "typescript" else "Not supported")
        self.assertIn("No additional language/toolchain lane is admitted", section)
        self.assertIn("169", section)
        self.assertIn("not_attempted", section)


class AcquisitionCapabilityTests(unittest.TestCase):
    """These fail-closed checks run even without POSIX source acquisition."""

    def test_unavailable_acquisition_never_reads_source_or_returns_a_plan(self):
        with mock.patch.object(scope, "acquisition_available", return_value=False), \
             mock.patch.object(p, "source_snapshot") as snapshot:
            with self.assertRaisesRegex(p.Error, "supported_scope_acquisition_unavailable"):
                scope.report()
            snapshot.assert_not_called()

    def test_missing_descriptor_relative_open_has_no_pathname_fallback(self):
        with mock.patch.object(os, "supports_dir_fd", set(), create=True), \
             mock.patch.object(p, "source_snapshot") as snapshot:
            self.assertFalse(scope.acquisition_available())
            with self.assertRaisesRegex(p.Error, "supported_scope_acquisition_unavailable"):
                scope.report()
            snapshot.assert_not_called()

    def test_missing_nofollow_flag_cannot_be_treated_as_zero_and_ignored(self):
        with mock.patch.object(os, "O_NOFOLLOW", 0, create=True), \
             mock.patch.object(p, "source_snapshot") as snapshot:
            self.assertFalse(scope.acquisition_available())
            with self.assertRaisesRegex(p.Error, "supported_scope_acquisition_unavailable"):
                scope.report()
            snapshot.assert_not_called()

    def test_unavailable_platform_cli_emits_failure_not_a_successful_empty_inventory(self):
        stdout, stderr = io.StringIO(), io.StringIO()
        with mock.patch.object(scope, "acquisition_available", return_value=False), \
             contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            self.assertEqual(scope.main([]), 2)
        self.assertEqual(stdout.getvalue(), "")
        result = json.loads(stderr.getvalue())
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(result["execution"], "not_attempted")
        self.assertEqual(result["reason"], "supported_scope_acquisition_unavailable")


if __name__ == "__main__":
    unittest.main()
