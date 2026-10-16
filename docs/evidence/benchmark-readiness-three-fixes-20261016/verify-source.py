#!/opt/homebrew/bin/python3
"""Exact preimages and accounting contract retention; no harness/model execution."""
from pathlib import Path
import ast,hashlib,json,subprocess
E=Path(__file__).resolve().parent;R=E.parents[2];m=json.loads((E/'changes.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
def git(path):return subprocess.check_output(['git','show',m['base']+':'+path],cwd=R)
assertions=0
for row in m['rows']:
 old=(E/row['archive']).read_bytes();assert sha(old)==row['before_sha256'] and old==git(row['path'])
 current=(R/row['path']).read_text();restored=current
 for edit in reversed(row['replacements']):assert restored.count(edit['after'])==1;restored=restored.replace(edit['after'],edit['before'],1)
 assert restored.encode()==old
 tree=ast.parse(current)
 if '/test_' in row['path']:
  def old_assertions(text):
   return [ast.get_source_segment(text,n)for n in ast.walk(ast.parse(text))if isinstance(n,ast.Call)and isinstance(n.func,ast.Attribute)and n.func.attr.startswith('assert')]
  before=old_assertions(old.decode());after=old_assertions(current);assert before==after;assertions+=len(before)
# Whole reporting, frozen tasks and prompt producers remain unchanged.
unchanged=['benchmarks/cli-tokens-v1/codex_report.py','benchmarks/event-sim-tokens-v1/codex_report.py','benchmarks/cli-tokens-v1/SPEC.md','benchmarks/webapp-tokens-v1/SPEC.md','benchmarks/webapp-tokens-v2/SPEC.md','benchmarks/event-sim-tokens-v1/SPEC.md','benchmarks/cli-tokens-v1/live_campaign.py','benchmarks/event-sim-tokens-v1/campaign.py']
for p in unchanged:assert(R/p).read_bytes()==git(p)
# Same old usage, pricing, requested model and authored/fixed-context semantics.
contracts={
 'benchmarks/cli-tokens-v1/codex_campaign.py':['trace_usage','_usage','list_price_estimate','codex_command'],
 'benchmarks/webapp-tokens-v2/codex_campaign.py':['trace_usage','_usage','list_price_estimate','codex_command','prompt_for','summarize','_summarize'],
 'benchmarks/live_campaign_common.py':['tokenizer_metadata','authored_source_metrics','legacy_net_input_metrics']}
for p,names in contracts.items():
 old=git(p).decode();new=(R/p).read_text()
 def body(text,name):return ast.get_source_segment(text,next(n for n in ast.parse(text).body if isinstance(n,ast.FunctionDef)and n.name==name))
 for name in names:assert body(old,name)==body(new,name),(p,name)
for p in [R/'benchmarks/test_codex_accounting_readiness.py',E/'verify-source.py']:ast.parse(p.read_text())
proof=dict(status='PASS_SOURCE_ONLY_BENCHMARK_READINESS',base=m['base'],whole_preimages=len(m['rows']),all_old_test_assertions_preserved=assertions,whole_unchanged_task_report_prompt_files=len(unchanged),unchanged_named_accounting_functions=sum(map(len,contracts.values())),provider_calls=False,Cargo=False,ROOT_edits=False,offline_test_log='focused-verification.json',offline_test_count=66,full_Python_suite=False)
print(json.dumps(proof,sort_keys=True))
