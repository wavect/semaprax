import importlib.util,json,pathlib,shutil,tempfile,unittest
ROOT=pathlib.Path(__file__).parent;S=importlib.util.spec_from_file_location('rss',ROOT/'law16_boolean_negation_rss_capsule.py');M=importlib.util.module_from_spec(S);S.loader.exec_module(M)
class RssTests(unittest.TestCase):
 def test_current_checkout_routes_have_thirty_wrapper_bound_samples(self):
  v=M.review_current(ROOT/'evidence/law16-boolean-negation-peak-rss-v2');self.assertEqual(v['status'],'current_checkout_matched_rss_authenticated');self.assertEqual(v['routes']['bend_verdict']['samples'],30);self.assertIn('no RSS ratio',v['nonclaims'][1])
 def test_copied_capsule_keeps_original_commands_and_rejects_foreign_input(self):
  with tempfile.TemporaryDirectory() as directory:
   copied=pathlib.Path(directory)/'copied'
   shutil.copytree(ROOT/'evidence/law16-boolean-negation-peak-rss-v2',copied)
   self.assertEqual(M.review_current(copied)['status'],'current_checkout_matched_rss_authenticated')
   receipt=copied/'bend-verdict.json';value=json.loads(receipt.read_text())
   value['samples'][0]['command'][-2]='/unrelated/bend-input.bend'
   receipt.write_text(json.dumps(value))
   manifest_path=copied/'manifest.json';manifest=json.loads(manifest_path.read_text())
   manifest['bend_receipt']['bytes']=receipt.stat().st_size
   manifest['bend_receipt']['sha256']=M.digest(receipt)
   manifest_path.write_text(json.dumps(manifest))
   with self.assertRaisesRegex(ValueError,'command differs from bound provenance'):M.review_current(copied)
if __name__=='__main__':unittest.main()
