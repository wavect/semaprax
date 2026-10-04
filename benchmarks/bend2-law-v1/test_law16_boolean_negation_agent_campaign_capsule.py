import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location('campaign_capsule',ROOT/'law16_boolean_negation_agent_campaign_capsule.py')
CAPSULE=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(CAPSULE)
class CampaignCapsuleTests(unittest.TestCase):
 def test_nine_retained_pairs_bind_agent_events_and_routes(self):
  value=CAPSULE.review(ROOT/'evidence/law16-boolean-negation-agent-campaign-v1')
  self.assertEqual(value['status'],'nine_matched_pairs_authenticated')
  self.assertEqual(value['aggregate']['pairs'],9)
  self.assertEqual(value['pairs'][-1]['semaprax-scalar-v1']['attack'],'rejected')
if __name__=='__main__':unittest.main()
