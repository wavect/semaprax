"""Static contract tests for the owned macOS HR-07 runner; no Cargo starts."""
import copy
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "hot_reload_macos_cross_layer",
    pathlib.Path(__file__).with_name("macos_cross_layer_evidence.py"),
)
RUN = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = RUN
SPEC.loader.exec_module(RUN)


class Contract(unittest.TestCase):
    def test_manifest_binds_measured_faults_to_exact_selectors(self):
        manifest, cells = RUN.read_manifest()
        categories = {row["id"]: row for row in manifest["fault_categories"]}
        self.assertEqual(set(categories), set(RUN.SELECTORS) | set(RUN.UNAVAILABLE_FAULTS))
        for identifier, (_, selector) in RUN.SELECTORS.items():
            self.assertEqual(cells[identifier]["selector"], selector)
            self.assertEqual(categories[identifier]["selector"], selector)
            self.assertEqual(categories[identifier]["availability"], "selector-required")
        for identifier, reason in RUN.UNAVAILABLE_FAULTS.items():
            self.assertEqual(categories[identifier]["availability"], "unavailable")
            self.assertIsNone(categories[identifier]["selector"])
            self.assertEqual(categories[identifier]["reason"], reason)

    def test_every_rust_selector_names_an_existing_test_function(self):
        paths = {
            "root-lib": {
                "watcher": pathlib.Path("src/project/hot_reload_watcher.rs"),
                "session": pathlib.Path("src/project/hot_reload_tests.rs"),
            },
            "toolchain-lib": {
                "state-handoff": pathlib.Path("crates/semaprax-toolchain/src/source_live_cli/hr04_state_handoff_tests.rs"),
                "handoff-fault": pathlib.Path("crates/semaprax-toolchain/src/source_live_cli/hr04_handoff_fault_tests.rs"),
                "config": pathlib.Path("crates/semaprax-toolchain/src/source_live_cli/tests.rs"),
            },
            "source-agent-integration": {
                "integration": pathlib.Path("crates/semaprax-toolchain/tests/cli_help_surface_v1/source_agent_hot_reload.rs"),
            },
        }
        for identifier, (target, selector) in RUN.SELECTORS.items():
            if "hot_reload_watcher::tests::" in selector:
                source = paths[target]["watcher"]
            elif "hot_reload::tests::" in selector:
                source = paths[target]["session"]
            elif "hr04_state_handoff_tests::" in selector:
                source = paths[target]["state-handoff"]
            elif "hr04_handoff_fault_tests::" in selector:
                source = paths[target]["handoff-fault"]
            elif target == "source-agent-integration":
                source = paths[target]["integration"]
            else:
                source = paths[target]["config"]
            function = selector.rsplit("::", 1)[-1]
            self.assertIn(f"fn {function}(", (RUN.ROOT / source).read_text(), identifier)

    def test_manifest_rejects_selector_drift_and_requires_explicit_platform_rows(self):
        manifest, _ = RUN.read_manifest()
        broken = copy.deepcopy(manifest)
        broken["fault_categories"][0]["selector"] = "invented::test"
        with self.assertRaises(ValueError):
            RUN.validate_manifest(broken)
        broken = copy.deepcopy(manifest)
        del broken["platform_lanes"]
        with self.assertRaises(ValueError):
            RUN.validate_manifest(broken)
        self.assertEqual(manifest["platform_lanes"], RUN.PLATFORM_LANES)
        self.assertEqual(RUN.PLATFORM_LANES["Linux"]["status"], "unavailable")
        self.assertEqual(RUN.PLATFORM_LANES["Windows"]["status"], "unavailable")

    def test_dry_run_lists_serial_fault_inventory_without_claiming_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "plan.json"
            value = RUN.plan(5, 1, pathlib.Path(directory) / "target")
            output.write_text(json.dumps(value))
            report = json.loads(output.read_text())
        self.assertEqual(report["serial_order"], ["interpreter-a-b", *RUN.SELECTORS])
        self.assertEqual(set(report["uncovered_fault_categories"]), set(RUN.UNAVAILABLE_FAULTS))
        self.assertEqual(report["platform_lanes"]["macOS"]["status"], "will-run")
        self.assertEqual(report["platform_lanes"]["Linux"]["status"], "unavailable")


if __name__ == "__main__":
    unittest.main()
