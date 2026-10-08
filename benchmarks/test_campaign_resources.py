import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import campaign_resources as resources


class CampaignResourceTests(unittest.TestCase):
    def volumes(self, free=None):
        return [{"device": 1, "path": "/fixture", "free_bytes": free if free is not None else resources.MIN_FREE_BYTES * 2}]

    def test_low_disk_refuses_before_invoking_either_arm(self):
        called = []
        @resources.guarded_attempt
        def launch(repo, artifacts, commit, trial):
            called.append(trial)
            return trial
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes(1)):
            for arm in ("semaprax", "typescript"):
                with self.assertRaisesRegex(ValueError, "paid launch refused"):
                    launch(None, Path(directory), None, {"arm": arm, "number": 1})
        self.assertEqual(called, [])

    def test_mid_attempt_disk_loss_preserves_acceptance_and_cost(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()):
            monitor = resources.Monitor(Path(directory), "semaprax-01")
            with patch.object(resources, "snapshot", return_value=self.volumes(1)):
                row = monitor.finish({"status": "accepted", "list_price": {"estimated": 2}})
            self.assertEqual(row["status"], "accepted")
            self.assertEqual(row["list_price"], {"estimated": 2})
            self.assertTrue(row["resource_assessment"]["contaminated"])
            self.assertFalse(row["resource_assessment"]["clean_comparison_eligible"])
            self.assertFalse(monitor.reserve.exists())
            self.assertEqual(json.loads(monitor.receipt.read_text())["row"], row)

    def test_transient_enospc_stderr_disqualifies_even_with_restored_free_space(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()):
            artifacts = Path(directory)
            (artifacts / "transcripts").mkdir()
            (artifacts / "transcripts/typescript-01.stderr.txt").write_text("rollout: No space left on device\n")
            monitor = resources.Monitor(artifacts, "typescript-01")
            row = monitor.finish({"status": "not_accepted"})
            self.assertEqual(row["resource_assessment"]["incidents"][0]["kind"], "disk_error_in_retained_stderr")

    def test_post_launch_io_failure_leaves_emergency_row_and_unknown_usage(self):
        @resources.guarded_attempt
        def launch(repo, artifacts, commit, trial):
            raise OSError(28, "No space left on device")
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()):
            row = launch(None, Path(directory), None, {"arm": "semaprax", "number": 2})
            receipt = Path(row["resource_assessment"]["receipt"])
            self.assertEqual(json.loads(receipt.read_text())["row"]["number"], 2)
            self.assertTrue(row["runner_error"])
            self.assertEqual(row["observed"], {})
            self.assertEqual(row["list_price"], {})
            self.assertIsNone(row["provider_receipt_actual_usd"])

    def test_monitor_deduplicates_same_volume_and_clean_attempt_keeps_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            samples = resources.snapshot([path, path / "not-yet-created"])
            self.assertEqual(len(samples), 1)
            with patch.object(resources, "snapshot", return_value=self.volumes()):
                monitor = resources.Monitor(path, "calibration")
                row = monitor.finish({"status": "ready", "separate_from_trials": True})
            self.assertTrue(row["resource_assessment"]["clean_comparison_eligible"])
            self.assertTrue(monitor.receipt.exists())


if __name__ == "__main__":
    unittest.main()
