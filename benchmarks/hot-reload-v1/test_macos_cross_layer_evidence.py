"""Static contract tests for the source-attributed macOS HR-07 runner."""
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "macos_cross_layer_evidence", pathlib.Path(__file__).with_name("macos_cross_layer_evidence.py")
)
RUN = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = RUN
SPEC.loader.exec_module(RUN)


class Contract(unittest.TestCase):
    def test_exact_manifest_inventory_binds_each_owned_selector(self):
        _, cells = RUN.read_manifest()
        self.assertEqual(set(RUN.SELECTORS), {
            "watcher-a-b-invalid-c", "watcher-invalid-c-repair", "source-agent-a-b",
            "source-agent-a-b-c", "prepared-worker-a-b-c-identity", "watcher-stop-resource-release",
        })
        self.assertIn("fixture resource released", cells["watcher-stop-resource-release"]["requires"])
        self.assertEqual(cells["native-process-identity"]["selector"], None)

    def test_plan_records_serial_nonzero_execution_and_limitations(self):
        with tempfile.TemporaryDirectory() as directory:
            target = RUN.ROOT / "target" / "static-contract"
            output = pathlib.Path(directory) / "plan.json"
            previous = sys.argv
            try:
                sys.argv = ["runner", "--target-dir", str(target), "--output", str(output), "--samples", "1", "--dry-run"]
                RUN.main()
            finally:
                sys.argv = previous
            value = json.loads(output.read_text())
        self.assertEqual(value["mode"], "plan")
        self.assertEqual(value["interpreter_samples"], 1)
        self.assertEqual(value["serial_order"], ["interpreter-a-b", *RUN.SELECTORS])
        self.assertIn("native-or-Wasm state swap", value["nonclaims"])

    def test_documented_command_keeps_source_build_and_limitations_explicit(self):
        readme = (RUN.SUITE / "README.md").read_text()
        specification = (RUN.ROOT / "docs/HOT-RELOAD-BENCHMARK-V1.md").read_text()
        for document in (readme, specification):
            self.assertIn("macos-cross-layer-evidence.sh", document)
            self.assertIn("target/hr07-macos-evidence", document)
            self.assertIn("native/Wasm", document)
        self.assertIn("No committed\nreport currently says it has been executed on macOS.", readme)

    def test_private_target_and_exact_count_parser_fail_closed(self):
        with self.assertRaises(ValueError):
            RUN.private_target(pathlib.Path("/tmp/not-private"))
        output = b"running 1 test\ntest x ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out\n"
        match = RUN.SUMMARY.search(output.decode())
        self.assertEqual({k: int(v) for k, v in match.groupdict().items()}, {"passed": 1, "failed": 0, "ignored": 0, "measured": 0, "filtered": 17})


if __name__ == "__main__":
    unittest.main()
