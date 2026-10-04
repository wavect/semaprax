import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).parent
S=importlib.util.spec_from_file_location('cells',ROOT/'law16_remaining_cells_admission.py');M=importlib.util.module_from_spec(S);S.loader.exec_module(M)
class RemainingCellsAdmissionTests(unittest.TestCase):
 def test_three_controls_are_bound_and_not_relabelled(self):
  v=M.review();self.assertEqual(v['status'],'unsupported');self.assertEqual([x['id'] for x in v['cells']],['supported-list-theorem-v1','law-preserving-refactor-v1','law-breaking-agent-edit-v1']);self.assertEqual(v['cells'][0]['attack'],'empty-sort');self.assertIn('no i32',v['nonclaims'][1])
if __name__=='__main__':unittest.main()
