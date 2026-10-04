import copy
import json
import tempfile
import unittest
from pathlib import Path

import full_u32_guarded_i64_profile_v2 as profile_v2


class FullU32GuardedI64ProfileV2(unittest.TestCase):
    def test_retained_pins_controls_and_identity_boundaries(self):
        self.assertEqual(profile_v2.verify(), [])
        profile = json.loads(profile_v2.PROFILE.read_text())
        self.assertEqual(profile["reproduction"]["executable_sha256_status"],
                         "observed_during_replay; executable is not retained in this capsule")
        self.assertIn("matching the recorded SHA256", profile["reproduction"]["replay_requirement"])

    def test_rejects_promoting_i64_theorem_to_matched_law16_bend(self):
        profile = json.loads(profile_v2.PROFILE.read_text())
        profile["routes"][2]["disposition"] = "matched_cross_language_theorem"
        with tempfile.TemporaryDirectory() as directory:
            edited = Path(directory) / "profile.json"
            edited.write_text(json.dumps(profile))
            errors = profile_v2.verify(edited)
        self.assertTrue(any("LAW15 route was incorrectly presented" in error for error in errors))

    def test_rejects_promoting_law16_list_model_to_theorem_match(self):
        profile = json.loads(profile_v2.PROFILE.read_text())
        profile["routes"][1]["disposition"] = "matched_full_domain_theorem"
        with tempfile.TemporaryDirectory() as directory:
            edited = Path(directory) / "profile.json"
            edited.write_text(json.dumps(profile))
            errors = profile_v2.verify(edited)
        self.assertTrue(any("LAW16 list route was promoted" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
