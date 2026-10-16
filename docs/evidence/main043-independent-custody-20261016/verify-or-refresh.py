#!/usr/bin/env python3
"""Fixed Main043 source-only custody verifier; --write-endpoints is explicit."""
import argparse, copy, hashlib, json, re, subprocess, runpy
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);p.add_argument('--write-endpoints',action='store_true');a=p.parse_args();R=Path(a.repo).resolve()
A='a82e00cc83f99b16249181c45710d962598577db';M='163865806d2665047cd7121aa013a23eaf726b05';D='docs/evidence/main043-independent-custody-20261016'
def read(path):return (R/path).read_text()
def git(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=R).decode()
def sha(s):return hashlib.sha256(s.encode()).hexdigest()
def encoded(x):return json.dumps(x,indent=2,ensure_ascii=False)+'\n'
def endpoint(path,value):
 s=encoded(value)
 if a.write_endpoints:(R/path).write_text(s)
 else:assert read(path)==s,('stale endpoint',path)
proof=json.loads(read(D+'/static-proof.json'));owners=proof['owners'];assert len(owners)==9
assert proof['ours']==A and proof['incoming']==M
for i,o in enumerate(owners):
 assert read(o['table'])==git(A,o['table']),('original root table changed',o['table'])
 assert read(f'{D}/incoming/table-{i}.json')==git(M,o['table'])
 assert read(f'{D}/incoming/owners/owner-{i}.rs.txt')==git(M,o['owner'])
assert read('src/source_reflows/semantic_receivers.rs')==git(A,'src/source_reflows/semantic_receivers.rs')
for path in ['.github/workflows/ci.yml','scripts/ci-low-stack-trace.py','scripts/tests/test_ci_low_stack_trace.py']:
 assert read(path)==git(M,path),('incoming CI bytes',path)
for path in (R/D/'incoming/archives').rglob('*'):
 if path.is_file():
  logical=path.relative_to(R/D/'incoming/archives').as_posix();assert path.read_text()==git(M,logical)==read(logical)
repairs=json.loads(git(M,'src/source_reflows.json'))
semantic=json.loads(git(M,'docs/evidence/receiver-source-custody-v1/inverse.json'));live=json.loads(git(M,'docs/evidence/live-source-audit-custody-v1/inverse.json'))
assert len(semantic)==12 and len(live)==2
def invert(s,row):
 assert sha(s)==row['after_sha256']
 for e in row['inverse']:
  assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
 assert sha(s)==row['before_sha256'];return s
def ci(path):
 s=git(M,path)
 for rs in [semantic+live,repairs]:
  row=next((r for r in rs if r['path']==path),None)
  if row:s=invert(s,row)
 return s
checks=[];total=0
for i,o in enumerate(owners):
 original=json.loads(git(M,o['table']));rows=copy.deepcopy(original)
 for old,row in zip(original,rows):
  path=row['source'];current=read(path);expected=ci(path) if 'ci_str!' in git(M,o['owner']) else git(M,path)
  key='new' if o['variant'] in ['LegacyNominal','Selected'] else 'after'
  result=expected
  for e in (reversed(old['edits']) if o['reverse'] else old['edits']):
   assert result.count(e[key])==1;result=result.replace(e[key],e['before'],1)
  archive=old.get('archive',old.get('original'))
  if archive:
   candidates=[str(Path(o['table']).parent/archive),archive];archive=next(q for q in candidates if (R/q).exists());assert result==git(M,archive)==read(archive)
  else:assert result==''
  if current!=expected:
   bridge={key:current,'before':expected}
   if 'name' in old['edits'][0]:bridge['name']='Main043 complete current live endpoint'
   if o['reverse']:row['edits'].append(bridge)
   else:row['edits'].insert(0,bridge)
  if 'current_sha256' in row:row['current_sha256']=sha(current)
  actual=current
  for e in (reversed(row['edits']) if o['reverse'] else row['edits']):
   assert actual.count(e[key])==1;actual=actual.replace(e[key],e['before'],1)
  assert actual==result
  # The entire raw input must fail the unchanged unique inverse after mutation.
  mutated=current+' '
  for e in (reversed(row['edits']) if o['reverse'] else row['edits']):
   if mutated.count(e[key])!=1:break
   mutated=mutated.replace(e[key],e['before'],1)
  assert mutated!=result
  checks.append(dict(table=i,path=path,current_sha256=sha(current),incoming_sha256=sha(expected),original_sha256=sha(result)));total+=1
 endpoint(f'{D}/live/table-{i}.json',rows)
assert total==75
def project(row):
 actual=read(row['source']);assert sha(actual)==row['current_sha256']
 for e in row['edits']:
  assert actual.count(e['new'])==1;actual=actual.replace(e['new'],e['old'],1)
 assert sha(actual)==row['original_sha256'];return actual
inputs=[]
for row in semantic+live:
 path=row['path'];current=read(path);expected=git(M,path)
 before=invert(expected,row);base='docs/evidence/live-source-audit-custody-v1' if row in live else 'docs/evidence/receiver-source-custody-v1'
 assert before==read(base+'/'+row['archive'])==git(M,base+'/'+row['archive'])
 projected=dict(source=path,current_sha256=sha(current),original_sha256=sha(expected),edits=[dict(new=current,old=expected)] if current!=expected else [])
 assert project(projected)==expected;inputs.append(projected)
endpoint(D+'/current-to-main043-helper14.json',inputs)
extras=[]
for path in proof['historical_ci_extras']:
 current=read(path);expected=ci(path);row=dict(source=path,current_sha256=sha(current),original_sha256=sha(expected),edits=[dict(new=current,old=expected)])
 assert project(row)==expected;extras.append(row)
assert len(extras)==5;endpoint(D+'/historical-ci-extra-inputs.json',extras)
# Existing sealed descriptors retain exactly their paths and enum boundary.
path='src/source_reflows_merge/main043_custody.rs';source=read(path)
pattern=re.compile(r'(path: concat!\(env!\("CARGO_MANIFEST_DIR"\), "/([^"\n]+)"\), length: )\d+(, sha256: )\[[^\]]+\]')
count=0
def descriptor(m):
 global count
 name=m[2];assert name.startswith(D+'/');payload=read(name);assert len(payload.encode())<=5081115
 count+=1;return m[1]+str(len(payload.encode()))+m[3]+'['+', '.join(map(str,bytes.fromhex(sha(payload))))+']'
updated=pattern.sub(descriptor,source);assert count==14
if a.write_endpoints:(R/path).write_text(updated)
else:assert source==updated,'stale sealed descriptors'
guards=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
required=set(proof['new_modules'])|{v for o in owners for v in o['raw_inputs']}|{x['source'] for x in inputs}
for path in guards:
 source=read(path);literals=re.findall(r'include_str!\(\s*"([^"]+)"',source)
 for q in required-{path}:assert any(s.endswith('/'+q) for s in literals),(path,'missing raw source',q)
 # Exact original guard text is retained after authenticated additive include fragments.
 original=git(A,path)
 extension=runpy.run_path(str(R/D/'guard-extensions.py'))
 extension['verify_guard'](path,source,original,a.write_endpoints)
for i in [2,4,7]:
 text=read(owners[i]['definition']);assert 'historical_ci_extra' not in text and 'main043_input' not in text
# Preserve the original reader verbatim; only the two fixed sibling getters append.
reader=read('src/source_reflows_merge/runtime_json.rs');assert reader.startswith(git(A,'src/source_reflows_merge/runtime_json.rs'))
proof['inputs']=checks;endpoint(D+'/static-proof.json',proof)
bindings=json.loads(read(D+'/definition-bindings.json'))
for row in bindings['definitions']:
 payload=read(row['path'])
 if a.write_endpoints and row['path']=='src/source_reflows_merge/main043_custody.rs':
  row['bytes']=len(payload.encode());row['sha256']=sha(payload)
 else:assert row['bytes']==len(payload.encode()) and row['sha256']==sha(payload),('definition drift',row['path'])
extension=runpy.run_path(str(R/D/'guard-extensions.py'));extension['verify_recipe']()
assert bindings['verification_script_sha256']==sha(read(D+'/guard-extension-original/verify-or-refresh.py')),'original verifier source drift'
if a.write_endpoints:bindings['guard_extension_verifier_sha256']=sha(Path(__file__).read_text())
else:assert bindings['guard_extension_verifier_sha256']==sha(Path(__file__).read_text()),'extended verifier source drift'
endpoint(D+'/definition-bindings.json',bindings)
print(json.dumps(dict(status='MAIN043_SOURCE_ONLY_CUSTODY_PASS',rows=75,helper_controls=14,historical_extra_views=5,raw_owners=[2,4,7],all_three_raw_inventories=True,rust_executed=False,write_endpoints=a.write_endpoints)))
