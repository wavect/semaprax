import json
import tempfile
import unittest
from pathlib import Path
from codex_report import acceptance_times, completed_items, input_slices, recount, summarize


def row(arm, number, status='accepted', cost=1):
    keys = ('model_requests', 'raw_input_tokens', 'cached_input_tokens', 'cache_write_input_tokens',
            'legacy_net_input_tokens', 'output_tokens', 'final_authored_tokens_proxy', 'agent_wall_seconds', 'acceptance_wall_seconds')
    return dict.fromkeys(keys, 10) | {'arm': arm, 'number': number, 'status': status,
                                      'conditional_api_equivalent_usd': cost}


class ReportTests(unittest.TestCase):
    def test_uncached_complement_is_not_task_only_net_or_double_counted_cache_write(self):
        self.assertEqual(input_slices({'input_tokens':100,'cached_input_tokens':30,
            'cache_write_input_tokens':20}), {
            'uncached_input_tokens_excluding_cache_reads':70,'task_only_net_input_tokens':None})
        for raw,cached in [(100,None),(None,0),(100,101),(True,0),(100,-1)]:
            self.assertIsNone(input_slices({'input_tokens':raw,'cached_input_tokens':cached})[
                'uncached_input_tokens_excluding_cache_reads'])

    def test_outer_turns_and_item_categories_keep_failures_and_unknowns(self):
        first,failed = row('semaprax',1),row('semaprax',2,'failed')
        first.update(outer_cli_turns=1,uncached_input_tokens_excluding_cache_reads=70,
                     exec_completed_item_counts={'agent_message':2,'command_execution':3})
        failed.update(outer_cli_turns=2,uncached_input_tokens_excluding_cache_reads=40,
                      exec_completed_item_counts={'command_execution':4})
        data={'campaign':{'trial_order':['semaprax','semaprax']}}
        arm=summarize(data,[first,failed])['arms']['semaprax']
        self.assertEqual(arm['additional_metrics_per_accepted_task_including_failures'][
            'outer_cli_turns'],3)
        self.assertEqual(arm['additional_metrics_per_accepted_task_including_failures'][
            'uncached_input_tokens_excluding_cache_reads'],110)
        self.assertEqual(arm['exec_completed_item_counts_all_attempts'],
            {'agent_message':2,'command_execution':7})
        self.assertIsNone(arm['task_only_net_input_tokens'])
        failed['outer_cli_turns']=None
        failed['exec_completed_item_counts']=None
        arm=summarize(data,[first,failed])['arms']['semaprax']
        self.assertIsNone(arm['additional_metrics_all_attempts']['outer_cli_turns'])
        self.assertIsNone(arm['exec_completed_item_counts_all_attempts'])
        self.assertIsNone(completed_items([{'exec_completed_item_counts':{'bad':True}}]))

    def test_exec_turn_counts_survive_a_missing_rollout_without_usage_fabrication(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            stream=root/'exec.jsonl'
            stream.write_text(json.dumps({'type':'turn.completed','usage':{}})+'\n'+
                json.dumps({'type':'item.completed','item':{'type':'command_execution'}})+'\n')
            path=root/'results.json'
            path.write_text(json.dumps({'campaign':{'trial_order':['semaprax']},'calibration':{},
                'trials':[{'arm':'semaprax','number':1,'status':'failed','failure':'missing trace',
                           'transcript':str(stream),'observed':{}}]}))
            trial=recount(path)['trials'][0]
            self.assertEqual(trial['outer_cli_turns'],1)
            self.assertEqual(trial['exec_completed_item_counts'],{'command_execution':1})
            for key in ('model_requests','raw_input_tokens',
                        'uncached_input_tokens_excluding_cache_reads','conditional_api_equivalent_usd'):
                self.assertIsNone(trial[key])

    def test_acceptance_subprocess_sum_does_not_claim_full_wall(self):
        receipt = {'acceptance': {'build': {'seconds': 2}, 'candidate_tests': {'seconds': 3},
            'checks': [{'seconds': 4}], 'boundary_checks': [{'seconds': 5}]}}
        self.assertEqual(acceptance_times(receipt), (None, 14))
        receipt['acceptance_elapsed_seconds'] = 17
        self.assertEqual(acceptance_times(receipt), (17, 14))
        del receipt['acceptance']['checks'][0]['seconds']
        self.assertEqual(acceptance_times(receipt), (17, None))

    def test_per_accepted_wall_includes_rejected_attempts_and_unknown_is_null(self):
        data = {'campaign': {'trial_order': ['semaprax', 'typescript', 'typescript', 'semaprax']}}
        rows = [row('semaprax', 1), row('typescript', 1), row('typescript', 2), row('semaprax', 2, 'not_accepted')]
        arm = summarize(data, rows)['arms']['semaprax']
        self.assertEqual(arm['wall_seconds_per_accepted_task_including_failures']['agent_wall_seconds'], 20)
        rows[-1]['acceptance_wall_seconds'] = None
        arm = summarize(data, rows)['arms']['semaprax']
        self.assertIsNone(arm['wall_seconds_per_accepted_task_including_failures']['acceptance_wall_seconds'])

    def test_interrupted_paid_row_keeps_unknown_usage_cost_and_wall_time(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'results.json'
            path.write_text(json.dumps({'campaign': {'trial_order': ['semaprax', 'typescript']},
                'calibration': {}, 'trials': [{'arm': 'semaprax', 'number': 1,
                'status': 'failed', 'failure': 'disk exhaustion', 'observed': {},
                'resource_assessment': {'contaminated': True, 'clean_comparison_eligible': False}}]}))
            report = recount(path)
            trial = report['trials'][0]
            for key in ('raw_input_tokens', 'model_requests', 'conditional_api_equivalent_usd',
                        'agent_wall_seconds', 'acceptance_wall_seconds'):
                self.assertIsNone(trial[key])
            self.assertEqual(report['recorded_attempts'], 1)
            self.assertEqual(report['resource_contaminated_attempts'], 1)
            self.assertFalse(report['clean_comparison_eligible'])

    def test_resource_incident_keeps_cost_and_acceptance_but_forbids_clean_comparison(self):
        first = row('semaprax', 1, cost=2)
        second = row('typescript', 1)
        first['resource_assessment'] = {'contaminated': True, 'clean_comparison_eligible': False}
        second['resource_assessment'] = {'contaminated': False, 'clean_comparison_eligible': True}
        report = summarize({'campaign': {'trial_order': ['semaprax', 'typescript']}}, [first, second])
        self.assertTrue(report['complete'])
        self.assertFalse(report['clean_comparison_eligible'])
        self.assertEqual(report['arms']['semaprax']['accepted'], 1)
        self.assertEqual(report['arms']['semaprax']['conditional_api_equivalent_usd_per_accepted_task'], 2)

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
