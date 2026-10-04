import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).parent
S=importlib.util.spec_from_file_location('report',ROOT/'law16_current_report.py');R=importlib.util.module_from_spec(S);S.loader.exec_module(R)
class CurrentReportTests(unittest.TestCase):
 def test_report_preserves_ten_pairs_separate_timing_and_open_closure(self):
  v=R.render();self.assertEqual(v['status'],'incomplete');self.assertEqual(v['matched_boolean']['agent_pairs']['total_pairs'],10);self.assertTrue(any('no cross-route timing ratio' in row for row in v['nonclaims']));self.assertTrue(v['closure'].startswith('no:'));self.assertEqual(v['matched_boolean']['ordinary_and_nonproof_process_routes']['bend-ordinary']['fresh_process']['count'],30);self.assertEqual(v['matched_boolean']['ordinary_and_nonproof_process_routes']['semaprax-check']['repeat_process']['count'],30);self.assertEqual(v['matched_boolean']['bounded_proof_and_verdict_process_routes']['semaprax_z3']['fresh_process']['count'],30);self.assertEqual(v['matched_boolean']['proof_path_nonresult']['samples'],60)
if __name__=='__main__':unittest.main()
