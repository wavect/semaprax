#!/usr/bin/env python3
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);a=A.parse_args();R=a.repo.resolve();D=R/'docs/evidence/main-f1-prefix-lexical-subsumption-20261016';OLD=R/'docs/evidence/afba-presence-successor-20261016';sha=lambda b:hashlib.sha256(b).hexdigest();get=lambda ref,p:subprocess.check_output(['git','show',ref+':'+p],cwd=R)
BASE='b5eca55b96890e1a815f0d8d9e7b42c78d743b07';MAIN='f1f8b5943d784043d24b39daa35bef91aafe02da';C=json.loads((D/'arm-contract.json').read_text());count=0;views={}
for label,ref in [('sealedb5',BASE),('main-f1',MAIN)]:
 restored={};rows=json.loads((D/('current-to-'+label+'.json')).read_text())
 for row in rows:
  p=row['source'];b=(R/p).read_bytes()
  if p==C['selector_source'] and sha(b)==C['embedded_source_sha256']:b=b.replace(C['embedded_literal'].encode(),C['runtime_literal'].encode(),1)
  assert sha(b)==row['current_sha256'],p;s=b.decode()
  for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
  b=s.encode();assert sha(b)==row['original_sha256'];assert b==get(ref,p)==(D/'before'/label/p).read_bytes();restored[str((R/p).resolve())]=b;count+=1
 views[label]=restored
# Immutable predecessor packet and original predicate algorithms are unchanged.
for p in subprocess.check_output(['git','ls-tree','-r','--name-only',BASE,'--',str(OLD.relative_to(R))],cwd=R).decode().splitlines():assert(R/p).read_bytes()==get(BASE,p)
rb0,rt0,wt0=Path.read_bytes,Path.read_text,Path.write_text;restored=views['sealedb5'];argv=sys.argv[:]
def rb(p):return restored.get(str(p.resolve()),rb0(p))
def rt(p,*args,**kw):
 b=restored.get(str(p.resolve()));return b.decode(kw.get('encoding')or'utf-8')if b is not None else rt0(p,*args,**kw)
def wt(p,s,*args,**kw):assert p.resolve().is_relative_to(OLD);return len(s)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt
 for file,args in [('verify-source.py',['--repo',str(R)]),('verify-authority.py',[])]:
  sys.argv=[str(OLD/file)]+args
  with contextlib.redirect_stdout(io.StringIO()):runpy.run_path(str(OLD/file),run_name='__main__')
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;sys.argv=argv
assert sha((R/C['final_normalizer']).read_bytes())==C['final_normalizer_sha256'];root=(R/C['selector_source']).read_bytes();assert sha(root)==C['common_source_sha256'];assert sha(root.replace(C['runtime_literal'].encode(),C['embedded_literal'].encode(),1))==C['embedded_source_sha256']
with contextlib.redirect_stdout(io.StringIO()):runpy.run_path(str(D/'verify-component.py'),run_name='__main__')
for name in ['receiver-inverse12.json','compact-inverse7.json']:
 rows=json.loads((D/name).read_text());assert len(rows)==(12 if name.startswith('receiver')else 7)
 for row in rows:
  s=get(MAIN,row['path']).decode();assert sha(s.encode())==row['after_sha256']
  for e in row['inverse']:assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
  assert sha(s.encode())==row['before_sha256'];archive='receiver-source-custody-v1'if name.startswith('receiver')else'compact-staged-observations-v1';assert s==(R/'docs/evidence'/archive/row['archive']).read_text()
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_input_rows':count,'immutable_predecessor_packet':True,'unchanged_predecessor_source_and_authority_algorithms':True,'Main12_and_Main7_complete':True,'shipping_sequence_exact':True,'rust_executed':False}))
