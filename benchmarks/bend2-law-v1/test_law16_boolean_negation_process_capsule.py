import importlib.util
import pathlib
import unittest
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location("law16_boolean_negation_process_capsule",ROOT/"law16_boolean_negation_process_capsule.py")
CAPSULE=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(CAPSULE)
class ProcessCapsuleTests(unittest.TestCase):
 def test_all_matched_routes_and_assertion_retaining_controls_are_authenticated(self):
  result=CAPSULE.review(ROOT/"evidence/law16-boolean-negation-process-v1")
  self.assertEqual(result["status"],"matched_semantics_local_routes_authenticated")
  self.assertEqual(result["observations"]["raw_streams"],480)
  self.assertEqual(result["observations"]["bend_verdict_seeded_attack_rejections"],60)
  self.assertEqual(result["comparability"]["cross_route_timing"].split(":")[0],"not_reported")
if __name__=="__main__": unittest.main()
