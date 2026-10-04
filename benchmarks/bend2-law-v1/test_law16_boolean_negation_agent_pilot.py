import importlib.util
import pathlib
import unittest
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location('law16_boolean_negation_agent_pilot',ROOT/'law16_boolean_negation_agent_pilot.py')
PILOT=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(PILOT)
class BooleanNegationAgentPilotTests(unittest.TestCase):
 def test_single_pilot_binds_raw_events_budget_and_independent_routes(self):
  result=PILOT.review(ROOT/'evidence/law16-boolean-negation-agent-pilot-v1')
  self.assertEqual(result['status'],'one_matched_pilot_authenticated')
  self.assertEqual(result['lanes']['bend2']['chargeable_tokens'],16626)
  self.assertEqual(result['lanes']['semaprax-scalar-v1']['chargeable_tokens'],16711)
  self.assertEqual(result['cost_usage']['status'],'unavailable')
if __name__=='__main__': unittest.main()
