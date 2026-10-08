import unittest
from codex_report import summarize


def row(arm, number, status='accepted', cost=1):
    keys = ('model_requests', 'raw_input_tokens', 'cached_input_tokens', 'cache_write_input_tokens',
            'legacy_net_input_tokens', 'output_tokens', 'final_authored_tokens_proxy', 'agent_wall_seconds', 'acceptance_wall_seconds')
    return dict.fromkeys(keys, 10) | {'arm': arm, 'number': number, 'status': status,
                                      'conditional_api_equivalent_usd': cost}


class ReportTests(unittest.TestCase):
    def test_failed_attempt_cost_is_in_accepted_task_denominator(self):
        data = {'campaign': {'trial_order': ['semaprax', 'typescript', 'typescript', 'semaprax']}}
        result = summarize(data, [row('semaprax', 1, cost=2), row('typescript', 1),
                                  row('typescript', 2), row('semaprax', 2, 'not_accepted', 4)])
        self.assertTrue(result['complete'])
        arm = result['arms']['semaprax']
        self.assertEqual(arm['accepted'], 1)
        self.assertEqual(arm['failed_or_rejected'], 1)
        self.assertEqual(arm['conditional_api_equivalent_usd_per_accepted_task'], 6)
        self.assertEqual(arm['metrics_all_attempts']['raw_input_tokens']['total'], 20)

    def test_unknown_cost_does_not_become_zero_or_lower_bound(self):
        data = {'campaign': {'trial_order': ['semaprax', 'typescript', 'semaprax']}}
        result = summarize(data, [row('semaprax', 1), row('typescript', 1), row('semaprax', 2, 'failed', None)])
        self.assertIsNone(result['arms']['semaprax']['conditional_api_equivalent_usd_per_accepted_task'])
        self.assertIsNone(result['arms']['semaprax']['actual_billed_usd_per_accepted_task'])

    def test_partial_round_keeps_unlaunched_attempts_separate(self):
        data = {'campaign': {'trial_order': ['semaprax', 'typescript', 'typescript', 'semaprax']}}
        result = summarize(data, [row('semaprax', 1)])
        self.assertFalse(result['complete'])
        self.assertEqual(result['unlaunched_order'], ['typescript', 'typescript', 'semaprax'])
        self.assertEqual(result['arms']['typescript']['attempted'], 0)
        self.assertIsNone(result['arms']['typescript']['metrics_all_attempts']['model_requests'])
        with self.assertRaisesRegex(ValueError, 'order'):
            summarize(data, [row('typescript', 1)])


if __name__ == '__main__':
    unittest.main()
