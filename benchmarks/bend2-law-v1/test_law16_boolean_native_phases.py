import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("native_phases", ROOT / "law16_boolean_native_phases.py")
PHASES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PHASES)


class NativePhaseTests(unittest.TestCase):
    def test_retained_pilot_and_thirty_repetitions_are_distinct(self):
        pilot = PHASES.review(ROOT / "evidence/law16-native-phase-pilot-v1")
        campaign = PHASES.review(ROOT / "evidence/law16-native-phase-thirty-v1")
        self.assertEqual(pilot["status"], "pilot_authenticated")
        self.assertEqual(campaign["status"], "thirty_repetitions_authenticated")
        self.assertEqual(set(campaign["summary"]), set(PHASES.PHASES))
        self.assertTrue(all(row["count"] == 30 for row in campaign["summary"].values()))

    def test_raw_runtime_drift_fails_review(self):
        with tempfile.TemporaryDirectory() as folder:
            copied = Path(folder) / "capsule"
            shutil.copytree(ROOT / "evidence/law16-native-phase-pilot-v1", copied)
            raw = copied / "raw/01-bend_run.stdout"
            raw.write_bytes(b"False\nFalse\n")
            with self.assertRaises(ValueError):
                PHASES.review(copied)


if __name__ == "__main__":
    unittest.main()
