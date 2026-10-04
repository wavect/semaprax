"""Contract tests for the cross-layer HR-07 runner; no Cargo or compiler starts."""
import copy
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("hot_reload_cross_layer", pathlib.Path(__file__).with_name("cross_layer.py"))
RUN = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = RUN
SPEC.loader.exec_module(RUN)


class Contract(unittest.TestCase):
    def test_manifest_retains_cross_layer_selectors_and_honest_unavailable_cells(self):
        manifest = json.loads(RUN.MANIFEST.read_text())
        RUN.validate(manifest)
        cells = {cell["id"]: cell for cell in manifest["cells"]}
        self.assertEqual(cells["native-process-identity"]["availability"], "unavailable")
        self.assertEqual(cells["native-or-wasm-state-swap"]["selector"], None)
        self.assertIn("A remains active", cells["watcher-a-b-invalid-c"]["requires"])
        self.assertIn("one admission after repair", cells["watcher-invalid-c-repair"]["requires"])
        self.assertIn("B to C replay without redispatch", cells["source-agent-a-b-c"]["requires"])

    def test_manifest_and_commands_fail_closed(self):
        manifest = json.loads(RUN.MANIFEST.read_text())
        broken = copy.deepcopy(manifest)
        broken["cells"][-1]["selector"] = "invented"
        with self.assertRaises(ValueError):
            RUN.validate(broken)
        with self.assertRaises(ValueError):
            RUN.parse_command('{"id":"source-agent-a-b","argv":[]}')
        self.assertEqual(RUN.summarize([1, 2, 3, 4, 5]), {"samples": 5, "median_ms": 3, "p95_ms": 5, "values_ms": [1, 2, 3, 4, 5]})

    def test_dry_run_writes_only_the_plan(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "plan.json"
            previous = sys.argv
            try:
                sys.argv = ["cross_layer.py", "--dry-run", "--output", str(output)]
                RUN.main()
            finally:
                sys.argv = previous
            value = json.loads(output.read_text())
        self.assertEqual(value["mode"], "plan")
        self.assertEqual(value["schema"], RUN.SCHEMA)
        self.assertEqual({cell["id"] for cell in value["cells"]}, {
            "interpreter-a-b", "watcher-a-b-invalid-c", "watcher-invalid-c-repair", "source-agent-a-b", "source-agent-a-b-c", "prepared-worker-a-b-c-identity", "watcher-stop-resource-release", "native-process-identity", "native-or-wasm-state-swap"})

    def test_supplied_selector_has_actual_samples_and_unsupplied_cells_stay_unavailable(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "report.json"
            command = json.dumps({"id": "source-agent-a-b", "argv": ["/usr/bin/true"]})
            previous = sys.argv
            try:
                sys.argv = ["cross_layer.py", "--samples", "2", "--selector-command", command, "--output", str(output)]
                RUN.main()
            finally:
                sys.argv = previous
            value = json.loads(output.read_text())
        measured = value["cells"]["source-agent-a-b"]
        self.assertEqual(measured["status"], "measured")
        self.assertEqual(measured["timing"]["samples"], 2)
        self.assertTrue(measured["output_digests"][0]["executable"].startswith("/"))
        self.assertTrue(measured["output_digests"][0]["executable_sha256"].startswith("sha256:"))
        self.assertEqual(value["cells"]["native-process-identity"]["status"], "unavailable")
        self.assertEqual(value["cells"]["interpreter-a-b"]["reason"], "no selector command was supplied")


if __name__ == "__main__":
    unittest.main()
