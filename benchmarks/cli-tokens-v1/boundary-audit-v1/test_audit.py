import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import audit


class BoundaryAuditTests(unittest.TestCase):
    def setUp(self):
        self.corpus = json.loads(audit.CORPUS.read_text())
        self.cases = {case['name']: case for case in self.corpus['cases']}

    def report(self, name):
        return json.loads(audit.expected(self.cases[name])[1])

    def test_utf8_order_and_exact_decimal_expected_facts(self):
        ranked = self.report('utf8-bytewise-tie-order')['top_paths']
        self.assertEqual([row['path'] for row in ranked], ['/\ue000', '/\U00010000'])
        large = self.report('decimal-above-js-safe-integer')
        self.assertEqual(large['bytes'], 18014398509481988)
        self.assertEqual(large['avg_bytes'], 6004799503160662)
        wide = self.report('decimal-sum-beyond-i64')
        self.assertEqual(wide['bytes'], 18446744073709551614)
        self.assertEqual(wide['avg_bytes'], 9223372036854775807)
        arbitrary = self.report('decimal-unbounded-token')
        self.assertEqual(arbitrary['bytes'], 123456789012345678901234567890123456790)

    def test_inclusive_file_size_final_byte_and_all_terminators(self):
        data, facts, nonempty = audit.materialize(self.cases['exactly-64-kib'])
        self.assertEqual(len(data), 65536)
        self.assertFalse(data.endswith(b'\n'))
        self.assertTrue(data.endswith(b'200 123'))
        self.assertTrue(data[:-1].endswith(b'200 12'))
        result = self.report('exactly-64-kib')
        self.assertEqual(result['bytes'], 123 * len(facts))
        self.assertEqual(result['lines'], nonempty)
        mixed, _, _ = audit.materialize(self.cases['half-up-and-all-line-terminators'])
        self.assertIn(b'\r\n', mixed)
        self.assertIn(b'\r203.', mixed)
        self.assertEqual(self.report('half-up-and-all-line-terminators')['error_rate'], 6.3)
        self.assertIn(b'"error_rate":6.3,', audit.expected(self.cases['half-up-and-all-line-terminators'])[1])

    def test_explicit_plus_and_path_grammar_control_facts(self):
        zones = self.report('literal-plus-timezone')
        self.assertEqual((zones['lines'], zones['requests'], zones['malformed']), (7, 2, 5))
        grammar = self.report('strict-request-and-path-grammar')
        self.assertEqual((grammar['requests'], grammar['malformed']), (1, 15))
        data, _, _ = audit.materialize(self.cases['strict-request-and-path-grammar'])
        self.assertIn(b'/quo"te', data)
        self.assertIn(b'/back\\slash', data)
        for case in self.corpus['cases']:
            text, encoded = audit.expected(case)
            self.assertTrue(text.endswith(b'\n'))
            self.assertTrue(encoded.endswith(b'\n'))
            self.assertNotIn(b'\n', encoded[:-1])
            self.assertEqual(list(json.loads(encoded)), ['lines', 'requests', 'malformed', 'unique_ips', 'status',
                'error_rate', 'bytes', 'avg_bytes', 'top_paths', 'hours', 'busiest_hour'])

    def test_incomplete_round_refuses_before_any_execution_or_copy(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            results = root / 'results.json'
            results.write_text(json.dumps({'campaign': {'trial_order': ['semaprax', 'typescript'] * 5}, 'trials': []}))
            with patch.object(audit, 'execute') as execute:
                with self.assertRaisesRegex(ValueError, 'completed matched'):
                    audit.audit(results, root / 'audit', 1, 1)
            execute.assert_not_called()
            self.assertFalse((root / 'audit').exists())

    def test_execution_mode_records_interpreter_without_native_only_disqualification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'run.sh').write_text('exec "$compiler" run "$source" -- "$@"\n')
            self.assertEqual(audit.execution_mode(root, 'semaprax')['mode'], 'interpreter_via_semaprax_run')
            (root / 'run.sh').write_text('exec "$DIR/loglens" "$@"\n')
            (root / 'loglens').write_bytes(b'\xcf\xfa\xed\xfefixture')
            self.assertEqual(audit.execution_mode(root, 'semaprax')['mode'], 'native_executable_via_wrapper')
            (root / 'run.sh').write_text('node "$DIR/dist/loglens.js" "$@"\n')
            self.assertEqual(audit.execution_mode(root, 'typescript')['mode'], 'node_via_wrapper')

    def test_cost_denominator_keeps_every_paid_failure_and_unknown_receipts(self):
        rows = [{'arm': 'semaprax', 'historical_status': 'accepted', 'expanded_qualification_passed': good,
                 'saved_conditional_api_equivalent_usd': cost} for good, cost in ((True, 1.0), (False, 2.0))]
        summary = audit.summarize(rows, 10)['arms']['semaprax']
        self.assertEqual(summary['attempted'], 2)
        self.assertEqual(summary['historical_33_check_accepted'], 2)
        self.assertEqual(summary['expanded_qualification_accepted'], 1)
        self.assertEqual(summary['saved_conditional_api_equivalent_usd_all_attempts'], '3.0')
        self.assertEqual(summary['conditional_api_equivalent_usd_per_expanded_accepted_task'], '3.0')
        self.assertIsNone(summary['actual_billing_receipt_usd'])
        partial = audit.summarize(rows[:1], 10, rows)['arms']['semaprax']
        self.assertEqual(partial['attempted'], 2)
        self.assertEqual(partial['not_yet_audited'], 1)
        self.assertEqual(partial['saved_conditional_api_equivalent_usd_all_attempts'], '3.0')
        for row in rows:
            row['expanded_qualification_passed'] = False
        self.assertIsNone(audit.summarize(rows, 10)['arms']['semaprax']['conditional_api_equivalent_usd_per_expanded_accepted_task'])


if __name__ == '__main__':
    unittest.main()
