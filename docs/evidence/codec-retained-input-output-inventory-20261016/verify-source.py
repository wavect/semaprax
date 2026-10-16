#!/opt/homebrew/bin/python3
"""Whole-source recipe, v1 contract and actual focused receipt verification only."""
from pathlib import Path
import ast,hashlib,json,re,subprocess
E=Path(__file__).resolve().parent;R=E.parents[2];M=json.loads((E/'changes.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
def git(p):return subprocess.check_output(['git','show',M['base']+':'+p],cwd=R)
current_sources=[]
for row in M['rows']:
 before=(E/row['archive']).read_bytes();assert before==git(row['path']) and sha(before)==row['before_sha256']
 current=(R/row['path']).read_text();assert sha(current.encode())==row['current_sha256'];restored=current
 for edit in reversed(row['replacements']):assert restored.count(edit['after'])==1;restored=restored.replace(edit['after'],edit['before'],1)
 assert restored.encode()==before;ast.parse(current);current_sources.append(dict(path=row['path'],sha256=row['current_sha256']))
# Compiler-receipt validation/refusals, original final-file classification, tasks,
# actual compiler invocation/publication semantics and complete old owners stay exact.
unchanged=['benchmarks/compiler_output_provenance.py','benchmarks/authored_source_recount.py',
'benchmarks/cli-tokens-v1/SPEC.md','benchmarks/webapp-tokens-v2/SPEC.md','benchmarks/event-sim-tokens-v1/SPEC.md',
'src/cli/json_codec.rs','benchmarks/cli-tokens-v1/test_live_campaign.py','benchmarks/cli-tokens-v1/test_codex_report.py',
'benchmarks/event-sim-tokens-v1/test_codex_report.py','benchmarks/webapp-tokens-v2/test_codex_campaign.py']
for p in unchanged:assert(R/p).read_bytes()==git(p),p
# Preserve every original accounting expression/assertion outside the exact additive recipes.
def assertions(text):
 return [ast.get_source_segment(text,n)for n in ast.walk(ast.parse(text))if isinstance(n,ast.Assert)]
for row in M['rows']:assert assertions((E/row['archive']).read_text())==assertions((R/row['path']).read_text())
tests=json.loads((E/'test-receipts.json').read_text());assert tests['total']==44
for row in tests['groups']:
 raw=(E/row['path']).read_bytes();assert sha(raw)==row['sha256'];text=raw.decode();assert text.rstrip().endswith('OK');assert int(re.search(r'Ran (\d+) tests?',text)[1])==row['tests']
assert sum(x['tests']for x in tests['groups'])==44
new='benchmarks/test_retained_codec_inventory.py';ast.parse((R/new).read_text());current_sources.append(dict(path=new,sha256=sha((R/new).read_bytes())))
print(json.dumps(dict(status='PASS_SOURCE_ONLY_RETAINED_CODEC_INVENTORY',base=M['base'],whole_preimages=4,whole_unchanged_validator_task_invocation_owner_files=len(unchanged),current_sources=current_sources,actual_focused_offline_tests=44,provider_calls=False,compiler_execution=False,Cargo=False,ROOT_edits=False,full_Python_suite=False,full_input_closure='NOT_ESTABLISHED',independent_execution_and_repeat='NOT_ESTABLISHED',OPT606='OPEN_PENDING_COMPILER_RECEIPT_AND_ACTUAL_QUALIFICATION'),sort_keys=True))
