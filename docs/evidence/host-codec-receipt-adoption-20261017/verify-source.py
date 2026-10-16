#!/usr/bin/env python3
"""Exact source preservation plus bounded offline receipt identities, no live runs."""
from pathlib import Path
from collections import Counter
import ast,hashlib,json,copy
E=Path(__file__).resolve().parent;R=E.parents[2];M=json.loads((E/'source-cuts.json').read_text())
def sha(data):return hashlib.sha256(data).hexdigest()
def restore(row,data):
 assert sha(data)==row['current_sha256']
 lines=data.decode().splitlines(keepends=True)
 for edit in reversed(row['edits']):
  assert ''.join(lines[edit['start']:edit['end']])==edit['current']
  lines[edit['start']:edit['end']]=edit['original'].splitlines(keepends=True)
 original=''.join(lines).encode();assert sha(original)==row['sha256'];assert original==(E/row['archive']).read_bytes();return original
originals={row['path']:restore(row,(R/row['path']).read_bytes())for row in M['rows']}
for row in M['unchanged']+M['new_sources']:assert sha((R/row['path']).read_bytes())==row['sha256'],row['path']
asserts=0
for path,raw in originals.items():
 old=ast.parse(raw);new=ast.parse((R/path).read_text())
 before=Counter(ast.dump(x)for x in ast.walk(old)if isinstance(x,ast.Assert));after=Counter(ast.dump(x)for x in ast.walk(new)if isinstance(x,ast.Assert));assert not before-after;asserts+=sum(before.values())
 if path.endswith('codex_campaign.py'):
  functions=lambda tree:{x.name:ast.dump(x)for x in tree.body if isinstance(x,(ast.FunctionDef,ast.AsyncFunctionDef,ast.ClassDef))}
  assert functions(old)==functions(new)
  def inventory(tree):return [ast.dump(v)for v in next(x.value.elts for x in tree.body if isinstance(x,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='HARNESS_SOURCE_FILES'for t in x.targets))]
  added=list(inventory(new));member=ast.dump(ast.Constant(value='benchmarks/compiler_retained_receipt.py'));assert added.count(member)==1;added.remove(member);assert added==list(inventory(old))
 else:
  functions=lambda tree:{x.name:ast.dump(x)for x in tree.body if isinstance(x,ast.FunctionDef)}
  a,b=functions(old),functions(new)
  for name in ['_options','_selection','_under','_request','_measurement','authoring_compiler']:assert a[name]==b[name],name
  calls=lambda tree:[ast.dump(x)for x in ast.walk(tree)if isinstance(x,ast.Call)and isinstance(x.func,ast.Attribute)and ast.unparse(x.func)in ['subprocess.run','os.execv','os.kill']]
  assert calls(old)==calls(new)
controls=0
for row in M['rows']:
 data=(R/row['path']).read_bytes()
 for bad in [data+b' ',data[:-1],data[:5]+b'!'+data[6:]]:
  try:restore(row,bad)
  except AssertionError:controls+=1
  else:raise AssertionError('source drift accepted')
 bad=copy.deepcopy(row);bad['edits'][0]['original']+=' '
 try:restore(bad,data)
 except AssertionError:controls+=1
 else:raise AssertionError('forged inverse accepted')
preserved_tests=sum(sum(isinstance(node,ast.Call)and isinstance(node.func,ast.Attribute)and node.func.attr.startswith('assert')for node in ast.walk(ast.parse((R/row['path']).read_text())))for row in M['unchanged']if row['path'].endswith('.py') and '/test' in row['path'])
initial=json.loads((E/'initial-source-binding.json').read_text())
for path,digest in initial['files'].items():assert sha((E/'initial-main7f-f3'/path).read_bytes())==digest
T=json.loads((E/'test-receipts.json').read_text())
for row in T['logs']:
 data=(E/row['path']).read_bytes();assert sha(data)==row['sha256'];assert b'\nOK\n' in data
 count=T[row['label']]['tests'];assert ('Ran '+str(count)+' tests').encode()in data
for filename,key in [('compiler_retained_receipt.py','new_helper_sha256'),('test_compiler_retained_receipt.py','new_tests_sha256')]:assert sha((R/'benchmarks'/filename).read_bytes())==T['final8'][key]
assert sha((E/'tests/initial28-helper.py.txt').read_bytes())==T['initial28']['new_helper_sha256']
assert sha((E/'tests/initial28-controls.py.txt').read_bytes())==T['initial28']['new_tests_sha256']
print(json.dumps({'status':'PASS_SOURCE_AND_FOCUSED_OFFLINE_ONLY','complete_current_preimages':len(M['rows']),'new_sources':len(M['new_sources']),'unchanged_whole_owners':len(M['unchanged']),'original_changed_source_assertions_preserved':asserts,'unchanged_test_assertions_preserved':preserved_tests,'drift_controls_rejected':controls,'offline_initial28':'PASS','offline_final8':'PASS','compiler_provider_live_execution':'NOT_RUN'},indent=2))
