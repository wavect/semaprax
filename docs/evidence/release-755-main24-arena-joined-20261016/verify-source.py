#!/usr/bin/env python3
"""Finite complete source domains. Original component algorithms remain unchanged."""
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);R=p.parse_args().repo.resolve();D=R/'docs/evidence/release-755-main24-arena-joined-20261016';sha=lambda b:hashlib.sha256(b).hexdigest();CO=subprocess.check_output;RUN=subprocess.run
components={
 'previous2c':('2c4306e1b19c9899a4b11abe7ca1e604f91fde15','f1-pieces-compile-joined-20261016'),
 'component755':('09b2f0643461506be1fdbb5add8a6e5fce770284','path-boundary-755-reconciliation-2c430-20261016'),
 'component24':('47eeb5faa8c8589cf026630bf89b6851d4ef54f7','main24-prefix-independent-next-20261016'),
 'component854':('c24368219fa0091c3d9d71d328fbc1896c65845d','arena-acceptance-controls-854-next-20261016'),
 'joint24':('89024ae1100ae858024f529c0f934ff2bf7955cd','main24-755-joint-next-20261016')}
views={};count=0
for label,(ref,packet) in components.items():
 view={}
 for row in json.loads((D/('current-to-'+label+'.json')).read_text()):
  path=row['source'];b=(R/path).read_bytes();assert sha(b)==row['current_sha256'],(label,path,'current');s=b.decode()
  for e in row['edits']:assert s.count(e['new'])==1,(label,path,'unique');s=s.replace(e['new'],e['old'],1)
  b=s.encode();assert sha(b)==row['original_sha256'];assert b==CO(['git','show',ref+':'+path],cwd=R)==(D/'before'/label/path).read_bytes();view[str((R/path).resolve())]=b;count+=1
 views[label]=view
 for path in CO(['git','ls-tree','-r','--name-only',ref,'--','docs/evidence/'+packet],cwd=R).decode().splitlines():assert (R/path).read_bytes()==CO(['git','show',ref+':'+path],cwd=R),(label,path,'immutable original packet')
# The new Main domain is explicitly scoped to twelve inputs; it is not a claim
# that all differences from Main are normalized by this independent domain.
main={}
for row in json.loads((D/'current-to-main24.json').read_text()):
 path=row['source'];s=(R/path).read_text();assert sha(s.encode())==row['current_sha256']
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert s.encode()==CO(['git','show','24ffa85e83d06d71f8214fdae9f631d0baea894e:'+path],cwd=R)==(D/'before/main24'/path).read_bytes();main[path]=s
assert len(main)==12
for table,n,packet in [('receiver-inverse12.json',12,'receiver-source-custody-v1'),('compact-inverse7.json',7,'compact-staged-observations-v1')]:
 rows=json.loads((R/'docs/evidence/main24-prefix-independent-next-20261016'/table).read_text());assert len(rows)==n
 for row in rows:
  s=main[row['path']];assert sha(s.encode())==row['after_sha256']
  for e in row['inverse']:assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
  assert sha(s.encode())==row['before_sha256'];assert s==(R/'docs/evidence'/packet/row['archive']).read_text()
rb0,rt0,wt0=Path.read_bytes,Path.read_text,Path.write_text;argv=sys.argv[:];results={}
for label,(ref,packet) in components.items():
 view=views[label];P=R/'docs/evidence'/packet
 def rb(p):return view.get(str(p.resolve()),rb0(p))
 def rt(p,*args,**kw):
  b=view.get(str(p.resolve()));return b.decode(kw.get('encoding')or'utf-8')if b is not None else rt0(p,*args,**kw)
 def wt(p,s,*args,**kw):assert p.resolve().is_relative_to(R/'docs/evidence');return len(s)
 def command(cmd,*args,**kw):
  # Evaluate Git namespace predicates at the same immutable component as the
  # complete source view. No original diff assertion is removed or relaxed.
  if isinstance(cmd,list) and cmd[:2]==['git','diff']:
   cmd=list(cmd)
   if 'HEAD'in cmd:cmd=[ref if x=='HEAD'else x for x in cmd]
   elif '--'in cmd:cmd.insert(cmd.index('--'),ref)
   else:cmd.append(ref)
  return CO(cmd,*args,**kw)
 def run(cmd,*args,**kw):
  if isinstance(cmd,list) and len(cmd)>1 and cmd[0]=='python3' and cmd[1].endswith('/main24-prefix-independent-next-20261016/verify-source.py'):
   old=sys.argv[:];sys.argv=cmd[1:]
   try:runpy.run_path(cmd[1],run_name='__main__')
   finally:sys.argv=old
   return subprocess.CompletedProcess(cmd,0)
  return RUN(cmd,*args,**kw)
 try:
  Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;subprocess.check_output=command;subprocess.run=run
  sys.argv=[str(P/'verify-source.py'),'--repo',str(R)];out=io.StringIO()
  with contextlib.redirect_stdout(out):runpy.run_path(str(P/'verify-source.py'),run_name='__main__')
  results[label]=out.getvalue()
 finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;subprocess.check_output=CO;subprocess.run=RUN;sys.argv=argv
source=(R/'src/source_reflows_merge.rs').read_text()
for name in ['main_dd0_previous','arena854_input','main7c6_input','main_afba_input','main_f1_input']:
 block=source[source.index('fn '+name+'('):];assert block.index('release_custody::input')<block.index('joined_custody::input')
assert 'release_custody::normalize(source, current)' in source
C=json.loads((D/'arm-contract.json').read_text());assert sha((R/C['final_normalizer']).read_bytes())==C['final_normalizer_sha256'];b=(R/C['selector_source']).read_bytes();assert sha(b)==C['common_source_sha256'];assert sha(b.replace(C['runtime_literal'].encode(),C['embedded_literal'].encode(),1))==C['embedded_source_sha256']
for row in json.loads((D/'new-owners.json').read_text()):
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256'] and len(b.splitlines())==row['lines']<=1500
 for file in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert (R/file).read_text().count(row['source'])==1,(file,row['source'])
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
CO(['git','merge-base','--is-ancestor','24ffa85e83d06d71f8214fdae9f631d0baea894e','HEAD'],cwd=R)
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_component_rows':count,'independent_main24_scope':12,'main24_receiver_and_compact_inverses':19,'original_algorithms_replayed':results,'new_owners_three_raw_unions':16,'rust_executed':False},sort_keys=True))
