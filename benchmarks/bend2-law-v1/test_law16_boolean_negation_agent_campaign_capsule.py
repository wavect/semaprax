import importlib.util,os,pathlib,tempfile,unittest
from unittest import mock
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location('campaign_capsule',ROOT/'law16_boolean_negation_agent_campaign_capsule.py')
CAPSULE=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(CAPSULE)
CAMPAIGN_SPEC=importlib.util.spec_from_file_location('campaign',ROOT/'law16_boolean_negation_agent_campaign.py')
CAMPAIGN=importlib.util.module_from_spec(CAMPAIGN_SPEC);CAMPAIGN_SPEC.loader.exec_module(CAMPAIGN)
class CampaignCapsuleTests(unittest.TestCase):
 def test_nine_retained_pairs_bind_agent_events_and_routes(self):
  value=CAPSULE.review(ROOT/'evidence/law16-boolean-negation-agent-campaign-v1')
  self.assertEqual(value['status'],'nine_matched_pairs_authenticated')
  self.assertEqual(value['aggregate']['pairs'],9)
  self.assertEqual(value['pairs'][-1]['semaprax-scalar-v1']['attack'],'rejected')
 def test_campaign_passes_the_explicit_codex_path_to_each_matched_trial(self):
  with tempfile.TemporaryDirectory() as directory:
   root=pathlib.Path(directory); codex=root/'pinned-codex';codex.write_bytes(b'fixture executable');codex.chmod(os.stat(codex).st_mode|0o100)
   calls=[]
   def capture(lane):
    def run(output,ordinal,executable):
     calls.append((lane,ordinal,executable));return {'trial':f'{lane}:{ordinal}'}
    return run
   with mock.patch.object(CAMPAIGN,'bend',side_effect=capture('bend')),mock.patch.object(CAMPAIGN,'sem',side_effect=capture('semaprax')):
    CAMPAIGN.main(['--output',str(root/'campaign'),'--first','2','--last','3','--codex',str(codex)])
   self.assertEqual(calls,[('bend',2,codex.resolve()),('semaprax',2,codex.resolve()),('bend',3,codex.resolve()),('semaprax',3,codex.resolve())])
 def test_campaign_refuses_a_missing_codex_path_before_creating_output(self):
  with tempfile.TemporaryDirectory() as directory:
   root=pathlib.Path(directory);output=root/'campaign'
   with self.assertRaises(SystemExit):
    CAMPAIGN.main(['--output',str(output),'--codex',str(root/'missing-codex')])
   self.assertFalse(output.exists())
if __name__=='__main__':unittest.main()
