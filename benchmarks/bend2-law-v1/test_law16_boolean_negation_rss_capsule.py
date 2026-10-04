import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).parent;S=importlib.util.spec_from_file_location('rss',ROOT/'law16_boolean_negation_rss_capsule.py');M=importlib.util.module_from_spec(S);S.loader.exec_module(M)
class RssTests(unittest.TestCase):
 def test_current_checkout_routes_have_thirty_wrapper_bound_samples(self):
  v=M.review_current(ROOT/'evidence/law16-boolean-negation-peak-rss-v2');self.assertEqual(v['status'],'current_checkout_matched_rss_authenticated');self.assertEqual(v['routes']['bend_verdict']['samples'],30);self.assertIn('no RSS ratio',v['nonclaims'][1])
if __name__=='__main__':unittest.main()
