"""Snapshot already actual #780 history only; never execute a compiler/test."""
from pathlib import Path
import hashlib,json,shutil
B=Path(__file__).resolve().parent;A=Path('/Volumes/Semaprax/provenance/closure-780-current-audit-20261015');assets={};raw_rows=[]
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def add(p,category):
 p=Path(p);digest=sha(p);key=category+'/'+digest[:16]+'-'+p.name
 if key not in assets:
  out=B/key;out.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,out);assert sha(out)==digest
  assets[key]={'path':key,'sha256':digest,'original_path':str(p),'bytes':p.stat().st_size}
 return key
for p in sorted(A.iterdir()):
 if p.suffix in ['.json','.md','.diff'] and p.name!='transport-callsites.txt':add(p,'audit')
rows=json.loads((A/'receipt-candidates.json').read_text())
for selector,candidates in rows.items():
 for x in candidates:
  receipt=Path(x['receipt']);data=json.loads(receipt.read_text());r=next(z for z in data['results'] if z.get('selector')==selector)
  assert sha(Path(r['log']))==r['log_sha256']
  raw_rows.append({'selector':selector,'source':data['source_subject'],'exit_code':r['exit_code'],'receipt_asset':add(receipt,'raw/receipts'),'log_asset':add(r['log'],'raw/logs'),'log_sha256':r['log_sha256'],'summary':r.get('summary')})
for o in json.loads((A/'four-additional-owning-pass-bindings.json').read_text()):
 for x in o['actual_passes']:
  assert sha(Path(x['receipt']))==x['receipt_sha256'];assert sha(Path(x['log']))==x['log_sha256']
  add(x['receipt'],'raw/receipts');add(x['log'],'raw/logs')
for x in json.loads((A/'scoped-clippy-bindings.json').read_text()):
 assert sha(Path(x['receipt']))==x['receipt_sha256'];assert sha(Path(x['log']))==x['log_sha256'];add(x['receipt'],'raw/clippy');add(x['log'],'raw/clippy')
manifest={'schema':'semaprax.opt780.closure-publication-preparation.v1','status':'PENDING_ACTUAL_CURRENT_WARM_CACHE_PASS','actual_current_PASS_claim':False,'historical_original_owner_count':24,'additional_existing_owner_count':4,'historical_original_rows':len(raw_rows),'historical_original_PASS':sum(x['exit_code']==0 for x in raw_rows),'historical_original_FAIL_retained':sum(x['exit_code']!=0 for x in raw_rows),'original_history':raw_rows,'assets':list(assets.values()),'audited_source_cut':'9ccc0e2f18e15d4d72f46262b7a940a23a66500e','observed_parent_source_before_pending_join':'236cfef3c2d3becf2f0d048862c4df10bee9ced1','final_source_binding':None,'required_actual_owner':{'package':'semaprax','kind':'test:project','harness':'semaprax::project','selector':'semantic_cache::byte_traversal::warm_byte_traversal_cache_preserves_mixed_graph_and_current_provider_refusals'},'runtime_dependency':'compile-time CARGO_MANIFEST_DIR/examples/frame-payload-project/{semaprax.toml,src/app.spx,src/frame.spx,src/tests.spx}; authenticate original build checkout, not cwd','no_positive_application_fit_or_benchmark_gate':True,'no_new_test_body_or_suite':True}
(B/'publication-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({k:manifest[k] for k in ['status','historical_original_rows','historical_original_PASS','historical_original_FAIL_retained']}));print('assets',len(assets),'bytes',sum(v['bytes'] for v in assets.values()))
