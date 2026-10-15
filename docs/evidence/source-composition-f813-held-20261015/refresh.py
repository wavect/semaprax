#!/usr/bin/env python3
"""Lossless layered successor. Prior manifests and runtime domains are immutable."""
from pathlib import Path
import sys,json,hashlib,subprocess,gzip,re
R=Path.cwd();D=Path(__file__).resolve().parent;P='docs/evidence/incremental-source-composition-5f1fecd-20261015'
sys.path.insert(0,str(R/P));from source_tools import Git,sha
G=Git(R);BASE='0ccaeef55025e50dd7566273b1e70bae8f3e3dfa';PRIOR='e4b90221df31aa994641d4748d539ec16ea814cb';MAIN='f813d2367114ee5c1738fd8239fdd3ae2e89e3fd';HELD='e6ee2462a6c0f469b6dc1e63f257bdafbc64eb0a'
def load(p):return json.loads((R/p).read_bytes())
def put(name,x):(D/name).write_text(json.dumps(x,indent=2)+'\n')
def raw(p):return (R/p).read_bytes()
def row(p,current,old,revision):
 assert current and old
 return dict(source=p,checkpoint_revision=revision,current_sha256=sha(current),original_sha256=sha(old),edits=[dict(new=current.decode(),old=old.decode())])
def apply(rows,p,b):
 found=[r for r in rows if r['source']==p];assert len(found)<=1
 if not found:return b
 r=found[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:assert b.count(e['new'].encode())==1;b=b.replace(e['new'].encode(),e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];assert b==G.get(r['checkpoint_revision'],p);return b
try:
 assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==BASE
 protected='docs/evidence/held-auth-839/source-inverses.json';assert sha(raw(protected))=='96ecd68d1a6367d8f3fe871d9fc5e14ebca69a02a9d3d263977936a65d844936'
 held=load(protected);heldrows=[]
 for r in held:
  p=r['path'];old=G.get(HELD,p);assert old.decode()==r['after'];assert sha(old)==r['after_sha256']
  if r['before'] is None:assert r['before_sha256'] is None
  else:assert sha(r['before'].encode())==r['before_sha256']
  if raw(p)!=old:heldrows.append(row(p,raw(p),old,HELD))
 put('held-projections.json',heldrows)
 def beforeheld(p):
  b=apply(heldrows,p,raw(p));rs=[r for r in held if r['path']==p]
  if rs:
   r=rs[0];assert sha(b)==r['after_sha256'];assert b.decode()==r['after']
   if r['before'] is not None:assert sha(r['before'].encode())==r['before_sha256'];return r['before'].encode()
  return b
 oldours=load('docs/evidence/source36-main88884-merge-20261015/ours-projections.json');oldfour=load('docs/evidence/source36-after-next-838-839-20261015/source-projections.json')
 pre=[]
 for p in sorted({r['source'] for r in oldours+oldfour}|set(re.findall(r'current_view::project\("([^"]+)"',raw('src/cleanup_plan/replay/live_oracle/source_custody/b.rs').decode()))):
  b=beforeheld(p);rs=[r for r in oldours if r['source']==p];old=rs[0]['edits'][0]['new'].encode() if rs else G.get('2b5cae170e13b16c4a5f57a619b302405e2934a6',p)
  assert old==G.get('2b5cae170e13b16c4a5f57a619b302405e2934a6',p),(p,'old input stage')
  if b!=old:pre.append(row(p,b,old,'2b5cae170e13b16c4a5f57a619b302405e2934a6'))
 put('pre-ours.json',pre)
 for p in {r['source'] for r in pre+oldours+oldfour}:
  stage=apply(oldours,p,apply(pre,p,beforeheld(p)));assert stage==G.get('6e549d1aead793166f7f601ce722cd43e1f0686d',p),(p,'actual Rust ours chain')
 p='src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs';old=G.get('2b5cae170e13b16c4a5f57a619b302405e2934a6',p);put('weighted-projections.json',[row(p,beforeheld(p),old,'2b5cae170e13b16c4a5f57a619b302405e2934a6')])
 ci=load('src/source_reflows.json');assert raw('src/source_reflows.json')==G.get(MAIN,'src/source_reflows.json')
 main=[]
 for p in sorted({r['path'] for r in ci}):
  old=G.get(MAIN,p)
  if raw(p)!=old:main.append(row(p,raw(p),old,MAIN))
 put('main-projections.json',main)
 for r in ci:
  b=apply(main,r['path'],raw(r['path']));assert sha(b)==r['after_sha256'],r['path']
  for e in r['inverse']:assert b.count(e['after'].encode())==1;b=b.replace(e['after'].encode(),e['before'].encode(),1)
  assert sha(b)==r['before_sha256']
 presence='src/cleanup_plan/replay/live_oracle/source_custody/presence.json';assert raw(presence)==G.get(MAIN,presence)
 def cip(p):
  b=apply(main,p,raw(p));rs=[r for r in ci if r['path']==p]
  for r in rs:
   assert sha(b)==r['after_sha256']
   for e in r['inverse']:assert b.count(e['after'].encode())==1;b=b.replace(e['after'].encode(),e['before'].encode(),1)
   assert sha(b)==r['before_sha256']
  return b
 for r in load(presence):
  b=cip(r['path']);assert sha(b)==r['after_sha256']
  for e in r['inverse']:assert b.count(e['after'].encode())==1;b=b.replace(e['after'].encode(),e['before'].encode(),1)
  assert sha(b)==r['before_sha256']
 # Preserve each prior active manifest by reference, adding one deduplicated full current→prior layer.
 prior=load(P+'/combined-source-proof.json');assert prior['total_packets']==101 and prior['total_rows']==904
 joins={};packets=[];priorfiles={}
 for entry in prior['packets']:
  p=entry['active_packet'];b=raw(p);assert b==G.get(PRIOR,p);priorfiles[p]=sha(b);data=json.loads(b);rows=data if isinstance(data,list) else data.get('whole_sources',data.get('owners',[data]));assert len(rows)==entry['rows']
  for r in rows:
   source=r.get('source',r.get('file',r.get('path')));old=G.get(PRIOR,source);assert old is not None
   if raw(source)!=old:joins[source]=row(source,raw(source),old,PRIOR)
  for child in data.get('children',[]) if isinstance(data,dict) else []:
   source=child['source'];old=G.get(PRIOR,source)
   if raw(source)!=old:joins[source]=row(source,raw(source),old,PRIOR)
  packets.append(dict(packet=entry['packet'],active_packet=p,rows=entry['rows'],sha256=sha(b),source_stage=PRIOR))
 # Include all current starter sources, receivers, and historical proof inputs in the same layer.
 for p in prior['source_inputs']:
  old=G.get(PRIOR,p)
  if raw(p)!=old:joins[p]=row(p,raw(p),old,PRIOR)
 for key in ['preserved_prior_proofs_and_scripts','unchanged_original_archives']:
  for p,h in prior[key].items():assert sha(raw(p))==h,(p,'immutable artifact')
 oldjoin=load(P+'/joined-source-proof.json');receiver=load(P+'/independent-receiver-bindings.json');unchanged=0
 for r in receiver['records']:
  p=r['source'];old=G.get(PRIOR,p)
  if raw(p)!=old:joins[p]=row(p,raw(p),old,PRIOR)
  original=G.get(r['checkpoint_revision'],p);assert sha(original)==r['original_sha256']
  unchanged+=raw(p)==original
 for r in load(P+'/starter-current-bindings.json'):
  p=r['source'];old=G.get(PRIOR,p)
  if raw(p)!=old:joins[p]=row(p,raw(p),old,PRIOR)
 put('current-to-prior.json',list(joins.values()))
 for p,r in joins.items():assert apply([r],p,raw(p))==G.get(PRIOR,p)
 # All 367 archived merge inputs authenticate independently, one file at a time.
 inputs=load('docs/evidence/main-f813-merge-20261015/inputs.json')
 for r in inputs:
  b=gzip.decompress(raw('docs/evidence/main-f813-merge-20261015/'+r['archive']));assert sha(b)==r['sha256'];assert b==G.get(r['commit'],r['path']);assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==r['git_blob']
 # Every test-only helper edit retains the complete immediate source inverse.
 inv=[]
 for p in ['src/source_reflows_merge.rs','src/cleanup_plan/replay/physical_budget/authentication/source.rs','src/cleanup_plan/replay/live_oracle/source_custody/b.rs','src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs']:
  inv.append(row(p,raw(p),G.get(BASE,p),BASE))
 put('source-inverses.json',inv)
 proof=dict(status='SOURCE_ONLY_NO_RUST_EXECUTION',source_basis=BASE,prior_metadata=PRIOR,prior_packets=packets,total_prior_packets=101,total_prior_rows=904,old_hook_count=oldjoin['old_hooks_preserved'],old_checkpoint_joins=oldjoin['authenticated_added_checkpoint_rows'],new_deduplicated_current_joins=len(joins),independent_receivers=53,independent_receivers_byte_unchanged=unchanged,independent_receivers_joined=53-unchanged,starter_bindings=38,starter_patch_paths=[32,6],next_patch_paths=66,held_runtime_rows=len(held),main_reflows=len(ci),merge_input_paths=len({r['path'] for r in inputs}),merge_input_rows=len(inputs),presence_rows=len(load(presence)),protected_held_sha256=sha(raw(protected)),protected_main_json_sha256=sha(raw('src/source_reflows.json')),protected_presence_sha256=sha(raw(presence)),old_runtime_domains_preserved=True,prior_artifact_count=sum(len(prior[k]) for k in ['preserved_prior_proofs_and_scripts','unchanged_original_archives']),prior_packet_sha256=priorfiles,git_cache_peak_bytes=G.peak)
 put('composition-proof.json',proof);print(json.dumps({k:v for k,v in proof.items() if k not in ['prior_packets','prior_packet_sha256']}))
finally:G.close()
