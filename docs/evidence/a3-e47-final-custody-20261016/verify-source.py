"""Finite complete source views; source-only, no Rust execution or runtime authority."""
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);R=p.parse_args().repo.resolve();D=R/'docs/evidence/a3-e47-final-custody-20261016';B='16c6b7f56b145ed085e4e7964caa3b1716159375';h=lambda b:hashlib.sha256(b).hexdigest();g=lambda *a:subprocess.check_output(['git',*a],cwd=R);views={}
for domain in json.loads((D/'domains.json').read_text()):
 view={};rows=json.loads((D/domain['table']).read_text());assert [r['source']for r in rows]==domain['keys']
 for r in rows:
  raw=(R/r['source']).read_bytes();assert h(raw)==r['current_sha256'],r['source'];s=raw.decode()
  for e in r['edits']:assert s.count(e['new'])==1,(domain['name'],r['source']);s=s.replace(e['new'],e['old'],1)
  b=s.encode();assert h(b)==r['original_sha256'];assert b==g('show',domain['commit']+':'+r['source'])==(D/'before'/domain['name']/r['source']).read_bytes();view[str((R/r['source']).resolve())]=b
 views[domain['name']]=view
for r in json.loads((R/'docs/evidence/maina3-independent-custody-20261016/table-domains.json').read_text()):
 assert (R/r['original_path']).read_bytes()==g('show',B+':'+r['original_path']);assert (R/r['Maina3_copy']).read_bytes()==g('show','a3cdfa095db21055558099ecef5e051b8472f6a3:'+r['original_path'])
for original,copy in [('docs/evidence/loan-endpoint-ordered-union-827/exact-source-inverses.json','loan-endpoint-inverses.json'),('docs/evidence/loan-inner-stages-829/exact-source-inverses.json','loan-inner-inverses.json')]:
 assert (R/original).read_bytes()==g('show','4251a9f6ba13db02c230f9bc547358d61434fbce:'+original)
 assert (R/'docs/evidence/maine47-independent-custody-20261016'/copy).read_bytes()==g('show','e47b64ffd25abc69545165f73b1ac5f9bb6eec82:'+original)
for r in json.loads((D/'new-owners.json').read_text()):
 b=(R/r['source']).read_bytes();assert h(b)==r['sha256'];assert len(b.splitlines())==r['lines']<=1500
 for f in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert(R/f).read_text().count(r['source'])>=1,(f,r['source'])
C=json.loads((D/'arm-contract.json').read_text());raw=(R/C['selector_source']).read_bytes();assert h(raw)==C['common_source_sha256'];assert raw.count(C['runtime_literal'].encode())==1;embedded=raw.replace(C['runtime_literal'].encode(),C['embedded_literal'].encode(),1);assert h(embedded)==C['embedded_source_sha256'];assert h((R/C['final_normalizer']).read_bytes())==C['final_normalizer_sha256']
O=R/'docs/evidence/main8a-independent-custody-20261016'
for path in g('ls-tree','-r','--name-only',B,'--',str(O.relative_to(R))).decode().splitlines():assert(R/path).read_bytes()==g('show',B+':'+path)
rb0,rt0,wt0=Path.read_bytes,Path.read_text,Path.write_text;argv=sys.argv[:];view=views['Previous16c']
def rb(p):return view[str(p.resolve())]if str(p.resolve())in view else rb0(p)
def rt(p,*a,**kw):return view[str(p.resolve())].decode(kw.get('encoding')or'utf-8')if str(p.resolve())in view else rt0(p,*a,**kw)
def wt(p,s,*a,**kw):assert p.resolve().is_relative_to(R/'docs/evidence');return len(s)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;sys.argv=[str(O/'verify-source.py'),'--repo',str(R)];out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(O/'verify-source.py'),run_name='__main__')
 prior=json.loads(out.getvalue());assert prior['status']=='PASS_SOURCE_ONLY'
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;sys.argv=argv
print(json.dumps({'status':'PASS_SOURCE_ONLY','independent_whole_source_views':{n:len(v)for n,v in views.items()},'protected_five_a3_two_e47_tables':True,'new_owners_all_three_raw_joins':len(json.loads((D/'new-owners.json').read_text())),'inherited16c_algorithm':prior['status'],'production_changed':True,'new_producer_required':True,'rust_executed':False},sort_keys=True))
