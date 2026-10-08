import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import codex_campaign
import live_campaign
import qualification


def settings():
    return {'round': 6, 'qualification': qualification.metadata(),
            'seed_files_sha256': live_campaign.ROUND_SEED_SHA256[6]}


def historical():
    return {'accepted': True, 'build': {'status': 'passed'},
            'candidate_tests': {'status': 'passed'},
            'checks': [{'name': str(i), 'status': 'passed'} for i in range(33)]}


class QualificationTests(unittest.TestCase):
    def test_plan_inventory_retains_frozen_inputs_and_new_round_identity(self):
        plan = settings()
        qualification.require_settings(plan)
        self.assertEqual(codex_campaign.ROUND, 6)
        self.assertEqual(live_campaign.ROUND_SEED_SHA256[6], live_campaign.ROUND_SEED_SHA256[5])
        self.assertEqual(plan['qualification']['boundary_checks'], 16)
        plan['qualification']['corpus_sha256'] = 'forged'
        with self.assertRaisesRegex(ValueError, 'inventory'):
            qualification.require_settings(plan)

    def test_changed_inventory_fails_before_candidate_execution_and_keeps_paid_row(self):
        plan = settings()
        plan['qualification']['facts_script_sha256'] = 'forged'
        with patch.object(live_campaign, 'check_program') as check:
            result = live_campaign.check_program_for_campaign(Path('candidate'), 1, {}, plan,
                'semaprax', Path('unused'))
        check.assert_not_called()
        self.assertFalse(result['accepted'])
        self.assertIn('inventory', result['qualification_failure'])

    def test_historical_round_gate_returns_exact_previous_object(self):
        old = historical()
        with patch.object(live_campaign, 'check_program', return_value=old), \
             patch.object(qualification, 'check') as boundary:
            result = live_campaign.check_program_for_campaign(Path('candidate'), 1, {},
                {'round': 5}, 'typescript', Path('unused'))
        self.assertIs(result, old)
        boundary.assert_not_called()

    def test_both_arms_use_identical_expected_bytes_and_single_failure_rejects(self):
        observations = []
        for arm, fail in [('semaprax', True), ('typescript', False)]:
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory); candidate = root / 'candidate'; candidate.mkdir()
                (candidate / 'run.sh').write_text('exec node app.mjs "$@"\n')
                output = root / 'qualification'
                def execute(command, cwd, env, logs, name, timeout):
                    expected = next(p for p in logs.glob('*.expected.*')
                        if p.name == name.rsplit('-', 1)[0] + '.expected.' + name.rsplit('-', 1)[1])
                    wanted = expected.read_bytes()
                    observations.append((arm, name, qualification.facts.sha(wanted), timeout))
                    actual = logs / f'{name}.stdout'; actual.write_bytes(wanted + (b'wrong' if fail and name == 'literal-plus-timezone-json' else b''))
                    return {'exit_code': 0, 'timed_out': False, 'elapsed_seconds': 0.01,
                            'stdout_path': str(actual)}
                with patch.object(qualification.facts, 'execute', side_effect=execute):
                    result = qualification.check(candidate, arm, {}, output, settings(), historical())
                self.assertTrue(result['historical_33_check_accepted'])
                self.assertEqual(result['accepted'], not fail)
                self.assertEqual(len(result['checks']), 33)
                self.assertEqual(len(result['boundary_checks']), 16)
                self.assertEqual(sum(c['status'] == 'failed' for c in result['boundary_checks']), int(fail))
                self.assertEqual(json.loads((output / 'qualification.json').read_text())['accepted'], not fail)
        self.assertEqual([x[1:] for x in observations if x[0] == 'semaprax'],
                         [x[1:] for x in observations if x[0] == 'typescript'])

    def test_empty_historical_corpus_or_boundary_timeout_cannot_accept(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); candidate = root / 'candidate'; candidate.mkdir()
            (candidate / 'run.sh').write_text('exec "$compiler" run source.spx\n')
            old = historical(); old['checks'] = []
            with patch.object(qualification.facts, 'execute', return_value={
                    'exit_code': -15, 'timed_out': True, 'elapsed_seconds': 120}):
                result = qualification.check(candidate, 'semaprax', {}, root / 'output', settings(), old)
            self.assertFalse(result['accepted'])
            self.assertEqual(len(result['boundary_checks']), 16)
            self.assertTrue(all(c['status'] == 'timeout' for c in result['boundary_checks']))


if __name__ == '__main__':
    unittest.main()
