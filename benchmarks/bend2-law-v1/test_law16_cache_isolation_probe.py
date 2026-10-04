#!/usr/bin/env python3
import importlib.util
import json
import pathlib
import shutil
import tempfile
import unittest

ROOT = pathlib.Path(__file__).parent
SCRIPT = ROOT / "law16_cache_isolation_probe.py"
SPEC = importlib.util.spec_from_file_location("law16_cache_isolation_probe", SCRIPT)
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)
RECEIPT = ROOT / "evidence/law16-cache-isolation-probe-v1"


class CacheIsolationProbeTests(unittest.TestCase):
    def test_retained_probe_receipt_authenticates_blocked_guest_control(self):
        result = PROBE.review(RECEIPT)
        self.assertEqual(result["cold_state"]["status"], "unavailable")
        self.assertFalse(result["guest_probe"]["drop_caches_writable"])
        self.assertEqual(result["guest_probe"]["measurement_samples"], 0)
        self.assertEqual(result["container_state"]["after_running"], 0)
        self.assertEqual(result["probe_image"]["digest"], PROBE.IMAGE_DIGEST)

    def test_review_rejects_a_cold_claim_without_measurements(self):
        with tempfile.TemporaryDirectory() as temporary:
            copied = pathlib.Path(temporary) / "receipt"
            shutil.copytree(RECEIPT, copied)
            path = copied / "receipt.json"
            receipt = json.loads(path.read_text())
            receipt["cold_state"]["status"] = "available_for_future_qualification"
            path.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "cannot qualify"):
                PROBE.review(copied)

    def test_review_rejects_changed_raw_probe_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            copied = pathlib.Path(temporary) / "receipt"
            shutil.copytree(RECEIPT, copied)
            path = copied / "raw/container-run-transcript.txt"
            path.write_bytes(path.read_bytes() + b"drop_caches_writable\n")
            with self.assertRaisesRegex(ValueError, "transcript digest or size drifted"):
                PROBE.review(copied)


if __name__ == "__main__":
    unittest.main()
