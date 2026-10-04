#!/usr/bin/env python3
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location('guest_cache', ROOT / 'law16_guest_cache.py')
CACHE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CACHE)
PILOT = ROOT / 'evidence/law16-guest-cache-pilot-v1'
CAMPAIGN = ROOT / 'evidence/law16-guest-cache-thirty-v1'


class GuestCacheTests(unittest.TestCase):
    def test_real_pilot_and_thirty_pair_capsules_authenticate_separate_cache_scope(self):
        self.assertEqual(CACHE.review(PILOT)['status'], 'pilot_authenticated')
        result = CACHE.review(CAMPAIGN)
        self.assertEqual(result['status'], 'thirty_guest_cache_pairs_authenticated')
        for lane in result['summary'].values():
            for state in lane.values():
                self.assertEqual(state['count'], 30)
                self.assertGreaterEqual(state['p95_ns'], state['p50_ns'])
        self.assertIn('host cache and Rosetta translation cache are unknown', result['nonclaims'][0])

    def test_physical_observation_cannot_be_relabelled_or_weakened(self):
        def nonzero_cold(value):
            value['samples'][0]['before_cold']['bun']['resident_pages'] = 1
        def missing_tool(value):
            for field in ('before_reset', 'before_cold', 'before_warm'):
                del value['samples'][0][field]['bun']
            del value['file_inventory']['bend_ordinary']['bun']
        def no_warm(value):
            value['samples'][0]['before_warm']['bun']['resident_pages'] = 0
        def wrong_scope(value):
            value['scope'] = 'host-cache-cold'
        def failed_child(value):
            value['samples'][0]['cold']['exit_code'] = 1
        for mutate in (nonzero_cold, missing_tool, no_warm, wrong_scope, failed_child):
            with self.subTest(mutation=mutate.__name__), tempfile.TemporaryDirectory() as directory:
                copy = Path(directory) / 'capsule'
                shutil.copytree(PILOT, copy)
                path = copy / 'guest-result.json'
                value = json.loads(path.read_text()); mutate(value)
                path.write_text(json.dumps(value))
                receipt_path = copy / 'receipt.json'
                receipt = json.loads(receipt_path.read_text())
                receipt['artifacts'] = [CACHE.ref(path, copy) if row['path'] == path.name else row for row in receipt['artifacts']]
                receipt_path.write_text(json.dumps(receipt))
                with self.assertRaises(ValueError):
                    CACHE.review(copy)

    def test_missing_effect_dependency_attempt_and_raw_drift_remain_nonresults(self):
        with self.assertRaises(ValueError):
            CACHE.review(ROOT / 'evidence/law16-guest-cache-pilot-failure-v1')
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory) / 'capsule'
            shutil.copytree(PILOT, copy)
            (copy / 'bend_ordinary-01-cold.stdout').write_bytes(b'fake pass\n')
            with self.assertRaisesRegex(ValueError, 'artifact identity drifted'):
                CACHE.review(copy)


if __name__ == '__main__':
    unittest.main()
