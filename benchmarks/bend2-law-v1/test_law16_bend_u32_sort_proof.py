"""Focused source/evidence binding controls; physical replay is separately retained."""
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import law16_bend_u32_sort_proof as proof


class BendU32SortProofTests(unittest.TestCase):
    def setUp(self):
        self.original = proof.BENCH / 'evidence/bend-u32-sort-universal-v1'

    def test_retained_actual_proof_and_negative_are_bound(self):
        capsule = proof.verify(self.original / 'capsule.json')
        self.assertEqual(capsule['commands']['empty_universal']['exit_code'], 1)
        self.assertEqual(capsule['commands']['kernel_replay']['exit_code'], 0)

    def test_success_cannot_replace_universal_negative(self):
        with tempfile.TemporaryDirectory() as directory:
            copied = Path(directory) / 'evidence'
            shutil.copytree(self.original, copied)
            path = copied / 'capsule.json'
            capsule = json.loads(path.read_text())
            capsule['commands']['empty_universal'] = capsule['commands']['candidate']
            path.write_text(json.dumps(capsule))
            with self.assertRaisesRegex(ValueError, 'count-law rejection'):
                proof.verify(path)

    def test_drifted_raw_kernel_evidence_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            copied = Path(directory) / 'evidence'
            shutil.copytree(self.original, copied)
            (copied / 'kernel-replay.stdout').write_text('ALL PROOFS CHECK\nforged\n')
            with self.assertRaisesRegex(ValueError, 'drift'):
                proof.verify(copied / 'capsule.json')


if __name__ == '__main__':
    unittest.main()
