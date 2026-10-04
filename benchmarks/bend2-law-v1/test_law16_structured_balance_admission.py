import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).parent
S=importlib.util.spec_from_file_location('balance',ROOT/'law16_structured_balance_admission.py');M=importlib.util.module_from_spec(S);S.loader.exec_module(M)
class StructuredBalanceAdmissionTests(unittest.TestCase):
 def test_binds_exact_u32_witness_and_pinned_parser_nonadmission(self):
  v=M.review();self.assertEqual(v['status'],'unsupported');self.assertEqual(v['semantic_witness']['after'],[6,7]);self.assertEqual(v['attack_witness']['after'],[9,4]);self.assertIn('not this checked-u32 cell',v['nonclaims'][0])
if __name__=='__main__':unittest.main()
