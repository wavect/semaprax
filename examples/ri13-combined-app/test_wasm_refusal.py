#!/usr/bin/env python3
import importlib.util
import pathlib
import tempfile
import unittest

MODULE = pathlib.Path(__file__).with_name("wasm-refusal.py")
SPEC = importlib.util.spec_from_file_location("ri13_wasm_refusal", MODULE)
REFUSAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REFUSAL)


class WasmRefusalTests(unittest.TestCase):
    def test_reviewed_profiles_are_exact_and_distinct(self):
        self.assertEqual(
            {name: REFUSAL.profile(manifest) for name, (manifest, _, _) in REFUSAL.PROJECTS.items()},
            {name: profile for name, (_, profile, _) in REFUSAL.PROJECTS.items()},
        )

    def test_refusal_requires_nonzero_exit_and_absent_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            output = root / "m1.wasm"
            failure = {"returncode": 1, "stdout": "", "stderr": "error[SPX-W114]: refused\n"}
            class Process:
                def __init__(self, row): self.returncode, self.stdout, self.stderr = row["returncode"], row["stdout"], row["stderr"]
            original = REFUSAL.subprocess.run
            REFUSAL.subprocess.run = lambda *args, **kwargs: Process(failure)
            try:
                row = REFUSAL.run_one(pathlib.Path("/bin/false"), root, "m1", REFUSAL.PROJECTS["m1"][0], "scalar-package", "refused")
            finally:
                REFUSAL.subprocess.run = original
            self.assertEqual(row["status"], "refused")
            self.assertEqual(row["diagnostic_codes"], ["SPX-W114"])
            self.assertFalse(row["output_exists"])

    def test_supported_scalar_requires_a_wasm_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            output = root / "m2.wasm"
            class Process:
                returncode = 0
                stdout = ""
                stderr = ""
            original = REFUSAL.subprocess.run
            def run(*args, **kwargs):
                output.write_bytes(b"\\0asm")
                return Process()
            REFUSAL.subprocess.run = run
            try:
                row = REFUSAL.run_one(pathlib.Path("/bin/true"), root, "m2", REFUSAL.PROJECTS["m2"][0], "implicit scalar.v1", "supported")
            finally:
                REFUSAL.subprocess.run = original
            self.assertEqual(row["status"], "supported")
            self.assertTrue(row["output_exists"])


if __name__ == "__main__":
    unittest.main()
