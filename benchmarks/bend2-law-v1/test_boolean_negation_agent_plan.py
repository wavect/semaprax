import hashlib
import importlib.util
import json
import pathlib
import unittest
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location("codex_agent_trial",ROOT/"codex_agent_trial.py")
RUNNER=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(RUNNER)
PLAN=ROOT/'evidence/law16-boolean-negation-agent-plan-v1.json'
class BooleanNegationAgentPlanTests(unittest.TestCase):
 def test_ten_matched_pairs_have_fixed_luna_budget_and_exact_mutants(self):
  plan=json.loads(PLAN.read_text()); RUNNER.require_boolean_negation_plan(plan); cell=plan['cells'][0]; trials=cell['trials']
  self.assertEqual(plan['status'],'preregistered_not_executed')
  self.assertEqual(plan['fixed_budget'],{'max_tokens':20000,'max_cost_usd':'0.00'})
  self.assertEqual(len(trials),20)
  self.assertEqual({t['id'] for t in trials},{f'boolean-negation-pair-v1:{language}:{ordinal}' for language in ('bend2','semaprax-scalar-v1') for ordinal in range(1,11)})
  self.assertEqual(plan['cost_usage']['status'],'unavailable_until_event')
 def test_runner_prompt_embeds_the_exact_negation_attack_without_repo_access(self):
  plan=json.loads(PLAN.read_text()); trial=next(t for t in plan['cells'][0]['trials'] if t['id']=='boolean-negation-pair-v1:bend2:1')
  context=RUNNER.edit_context(trial); prompt=RUNNER.prompt(trial)
  attack=(ROOT/'fixtures/bend-boolean-negation-law-gaming-v1.bend').read_bytes()
  self.assertEqual(context['attack_sha256'],'sha256:'+hashlib.sha256(attack).hexdigest())
  self.assertIn('boolean-negation-pair-v1',prompt)
  self.assertIn(context['attack_sha256'],prompt)
  self.assertEqual(trial['execution']['repository_access'],'none')
if __name__=='__main__': unittest.main()
