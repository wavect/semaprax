#!/usr/bin/env python3
"""Explicit component namespaces; unchanged original algorithms, no Rust execution."""
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);a=A.parse_args();R=a.repo.resolve();D=R/'docs/evidence/f1-pieces-compile-joined-20261016';sha=lambda b:hashlib.sha256(b).hexdigest();CO=subprocess.check_output
components={'mainf1':('253bc2442d2c2c4023c95267caf6d607839f4f23','main-f1-prefix-lexical-subsumption-20261016'),'pieces798':('9f171d4eb8a9e07c69fbc39ccc0faea87639aeb1','compiled-audit-pieces-798-next-20261016'),'compilefix':('bc6e1775a44fb04622d465cbd05a8171a04e1d07','b5eca-paid-family-compile-fixes-20261016')}
C=json.loads((D/'arm-contract.json').read_text());views={};count=0
for label,(ref,packet)in components.items():
 rows=json.loads((D/('current-to-'+label+'.json')).read_text());view={}
 for row in rows:
  path=row['source'];b=(R/path).read_bytes()
  if path==C['selector_source'] and sha(b)==C['embedded_source_sha256']:b=b.replace(C['embedded_literal'].encode(),C['runtime_literal'].encode(),1)
  assert sha(b)==row['current_sha256'],(label,path);s=b.decode()
  for e in row['edits']:assert s.count(e['new'])==1,(label,path);s=s.replace(e['new'],e['old'],1)
  b=s.encode();assert sha(b)==row['original_sha256'];assert b==CO(['git','show',ref+':'+path],cwd=R)==(D/'before'/label/path).read_bytes();view[str((R/path).resolve())]=b;count+=1
 views[label]=view
 for path in CO(['git','ls-tree','-r','--name-only',ref,'--','docs/evidence/'+packet],cwd=R).decode().splitlines():assert(R/path).read_bytes()==CO(['git','show',ref+':'+path],cwd=R),(label,path,'immutable original packet')
rb0,rt0,wt0=Path.read_bytes,Path.read_text,Path.write_text;argv=sys.argv[:];results={}
for label,(ref,packet)in components.items():
 view=views[label];P=R/'docs/evidence'/packet
 def rb(p):return view.get(str(p.resolve()),rb0(p))
 def rt(p,*args,**kw):
  b=view.get(str(p.resolve()));return b.decode(kw.get('encoding')or'utf-8')if b is not None else rt0(p,*args,**kw)
 def wt(p,s,*args,**kw):assert p.resolve().is_relative_to(P);return len(s)
 def command(cmd,*args,**kw):
  # Original798's final no-shipping/no-other-packet predicate evaluates its
  # authentic immutable component Git namespace, not the combined HEAD.
  if label=='pieces798' and isinstance(cmd,list) and cmd[:3]==['git','diff','--name-only'] and len(cmd)==4:cmd=cmd+[ref]
  return CO(cmd,*args,**kw)
 try:
  Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;subprocess.check_output=command
  files=['verify-source.py']+(['verify-authority.py']if label!='compilefix'else[]);results[label]={}
  for file in files:
   sys.argv=[str(P/file)]+(['--repo',str(R)]if file=='verify-source.py'else[]);out=io.StringIO()
   with contextlib.redirect_stdout(out):runpy.run_path(str(P/file),run_name='__main__')
   results[label][file]=json.loads(out.getvalue())
 finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;subprocess.check_output=CO;sys.argv=argv
assert (D/'generate-authority.py').read_bytes()==CO(['git','show',components['pieces798'][0]+':docs/evidence/compiled-audit-pieces-798-next-20261016/generate-authority.py'],cwd=R)
assert (D/'prepare-arm-overlay.py').read_bytes()==CO(['git','show',components['mainf1'][0]+':docs/evidence/main-f1-prefix-lexical-subsumption-20261016/prepare-arm-overlay.py'],cwd=R)
source=(R/'src/source_reflows_merge.rs').read_text()
for name in ['main_dd0_previous','arena854_input','main7c6_input','main_afba_input','main_f1_input']:
 block=source[source.index('fn '+name+'('):];assert block.index('joined_custody::input')<block.index('prefix_custody::input')
assert 'joined_custody::normalize(source, current)'in source
assert sha((R/C['final_normalizer']).read_bytes())==C['final_normalizer_sha256'];b=(R/C['selector_source']).read_bytes();assert sha(b)==C['common_source_sha256'];assert sha(b.replace(C['runtime_literal'].encode(),C['embedded_literal'].encode(),1))==C['embedded_source_sha256']
for row in json.loads((D/'new-owners.json').read_text()):assert sha((R/row['source']).read_bytes())==row['sha256']and len((R/row['source']).read_text().splitlines())==row['lines']<=1500
for file in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 s=(R/file).read_text();assert s.startswith('macro_rules! audit_source_join');assert s.count('src/source_reflows_merge/joined_custody.rs')==1
 for row in json.loads((R/'docs/evidence/main-f1-prefix-lexical-subsumption-20261016/new-owners.json').read_text()):assert s.count(row['source'])==1,(file,row['source'])
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_input_rows':count,'immutable_component_packets':True,'original_algorithms_replayed_on_authentic_component_namespaces':True,'pieces798_original_no_production_change_assertion_preserved':True,'component_results':results,'rust_executed':False,'memory_benefit_measured':False},sort_keys=True))
