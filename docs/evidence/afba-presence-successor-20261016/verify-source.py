#!/usr/bin/env python3
"""Finite complete-input successor replay; no Rust, admission or memory claim."""
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);a=A.parse_args();R=a.repo.resolve();D=R/'docs/evidence/afba-presence-successor-20261016';OLD=R/'docs/evidence/owned-session-arena-bulk-joined-20261016';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*args):return subprocess.check_output(['git',*args],cwd=R)
B=json.loads((D/'source-bindings.json').read_text());C=json.loads((D/'arm-contract.json').read_text());count=0;views={}
def normal(path,b):
 if path!=C['selector_source']:return b
 if sha(b)==C['common_source_sha256']:return b
 assert sha(b)==C['embedded_source_sha256'];assert b.count(C['embedded_literal'].encode())==1
 b=b.replace(C['embedded_literal'].encode(),C['runtime_literal'].encode(),1);assert sha(b)==C['common_source_sha256'];return b
for label,item in B['capsules'].items():
 rows=json.loads((D/item['path']).read_text());assert len(rows)==item['rows'] and sha((D/item['path']).read_bytes())==item['sha256'];restored={}
 for row in rows:
  path=row['source'];b=normal(path,(R/path).read_bytes());assert sha(b)==row['current_sha256'],(label,path);text=b.decode()
  for e in row['edits']:assert text.count(e['new'])==1,(label,path);text=text.replace(e['new'],e['old'],1)
  b=text.encode();assert sha(b)==row['original_sha256'];assert b==git('show',item['ref']+':'+path)==(D/'before'/label/path).read_bytes();restored[str((R/path).resolve())]=b;count+=1
 views[label]=restored
for row in B['protected724']:
 assert (R/row['path']).read_bytes()==git('show',B['sealed724']+':'+row['path']);assert sha((R/row['path']).read_bytes())==row['sha256']
for row in B['protected_presence']:
 assert (R/row['path']).read_bytes()==git('show',B['presence_ref']+':'+row['path']);assert sha((R/row['path']).read_bytes())==row['sha256']
for row in json.loads((D/'new-owners.json').read_text()):
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256'] and len(b.decode().splitlines())==row['lines']<=1500
assert sha((R/C['final_normalizer']).read_bytes())==C['final_normalizer_sha256']
# Execute the UNCHANGED predecessor source and authority algorithms against the
# authenticated complete 72432 input view. Writes are captured outside its packet.
read_bytes,read_text,write_text=Path.read_bytes,Path.read_text,Path.write_text
restored=views['sealed724'];captured={}
def rb(p):return restored.get(str(p.resolve()),read_bytes(p))
def rt(p,*args,**kwargs):
 b=restored.get(str(p.resolve()))
 return b.decode(kwargs.get('encoding')or'utf-8')if b is not None else read_text(p,*args,**kwargs)
def wt(p,data,*args,**kwargs):
 assert p.resolve().is_relative_to(OLD.resolve());captured[str(p.relative_to(OLD))]=data;return len(data)
output={};argv=sys.argv[:]
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt
 for name,args in [('verify-source.py',['--repo',str(R)]),('verify-authority.py',[])]:
  sys.argv=[str(OLD/name)]+args;out=io.StringIO()
  with contextlib.redirect_stdout(out):runpy.run_path(str(OLD/name),run_name='__main__')
  output[name]=json.loads(out.getvalue())
finally:Path.read_bytes=read_bytes;Path.read_text=read_text;Path.write_text=write_text;sys.argv=argv
# The original presence verifier executes against its independent six-input view.
try:
 restored=views['presence'];Path.read_bytes=rb;Path.read_text=rt;sys.argv=['verify-source.py','--repo',str(R)]
 out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(R/'docs/evidence/direct-presence-receiving-850-next-20261016/verify-source.py'),run_name='__main__')
 assert out.getvalue().strip()
finally:Path.read_bytes=read_bytes;Path.read_text=read_text;sys.argv=argv
assert (R/'docs/evidence/main-afba-receiver-source-custody-20261016/inverse.json').read_bytes()==git('show',B['main_afba']+':docs/evidence/receiver-source-custody-v1/inverse.json')
# Complete actual Main-afba12 source cuts and predecessor archives remain exact.
rows=json.loads((R/'docs/evidence/main-afba-receiver-source-custody-20261016/inverse.json').read_text());assert len(rows)==12
for row in rows:
 text=git('show',B['main_afba']+':'+row['path']).decode();assert sha(text.encode())==row['after_sha256']
 for e in row['inverse']:assert text.count(e['after'])==1;text=text.replace(e['after'],e['before'],1)
 assert sha(text.encode())==row['before_sha256'];assert text==(R/'docs/evidence/receiver-source-custody-v1'/row['archive']).read_text()
for row in B['presence_shipping']:
 assert (R/row).read_bytes()==git('show',B['presence_ref']+':'+row)
helper=(R/'src/source_reflows_merge.rs').read_text();assert helper.count('successor_custody::Fixture::Sealed724')==3
for endpoint in ['main_dd0_previous','arena854_input','main7c6_input']:
 block=helper[helper.index('fn '+endpoint):];assert block.index('successor_custody::input')<block.index('final_custody::input')
assert 'successor_custody::Fixture::MainAfba' in helper
assert (R/'tests/module-size-budget.tsv').read_bytes()==git('show',B['sealed724']+':tests/module-size-budget.tsv')
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_input_rows':count,'immutable724_files':len(B['protected724']),'predecessor_source_proof':output['verify-source.py'],'predecessor_authority_original_checks':True,'Main_afba12_complete_inverses':True,'presence_shipping_exact_component':True,'rust_executed':False,'memory_benefit_measured':False},sort_keys=True))
