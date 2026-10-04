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
        self.assertEqual({name: REFUSAL.profile(manifest) for name, (manifest, _) in REFUSAL.PROJECTS.items()}, {name: profile for name, (_, profile) in REFUSAL.PROJECTS.items()})

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
                row = REFUSAL.run_one(pathlib.Path("/bin/false"), root, "m1", REFUSAL.PROJECTS["m1"][0], "scalar-package")
            finally:
                REFUSAL.subprocess.run = original
            self.assertEqual(row["status"], "refused")
            self.assertEqual(row["diagnostic_codes"], ["SPX-W114"])
            self.assertFalse(row["output_exists_after_refusal"])


if __name__ == "__main__":
    unittest.main()
