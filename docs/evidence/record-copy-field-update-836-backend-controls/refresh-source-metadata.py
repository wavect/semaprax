"""Source-only coverage custody refresh; preserves all immediate originals."""
from pathlib import Path
import copy,hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='186854fff7cc556ac1852019f39420094adc9996';OLD=R/'docs/evidence/record-copy-field-update-836-backends'
sha=lambda b:hashlib.sha256(b).hexdigest()
def blob(p):return subprocess.check_output(['git','show',B+':'+p],cwd=R)
owners=['tests/support/record_copy_field_update.rs','tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs','tests/owned_data/owned_leaf_vec/copy_field_update.rs','tests/owned_data/owned_leaf_vec/host.js']
children=['tests/cleanup_backends/owned_leaf_collections/copy_field_update_boundary.rs','tests/cleanup_backends/owned_leaf_collections/copy_field_update_boundary.c']
rows=[]
for p in owners:
 old=blob(p);new=(R/p).read_bytes();ar=D/'original'/p.replace('/','__');ar.write_bytes(old)
 rows.append({'source':p,'basis':B,'archive':str(ar.relative_to(R)),'original_sha256':sha(old),'current_sha256':sha(new),'edits':[{'new':new.decode(),'before':old.decode()}]})
packet={'basis':B,'status':'SOURCE_AUTHORED_UNEXECUTED','owners':rows,'children':[{'source':p,'sha256':sha((R/p).read_bytes())}for p in children]}
(D/'source-inverses.json').write_text(json.dumps(packet,indent=2)+'\n')
old_inv_path=str(OLD.relative_to(R)/'source-inverses.json');old=blob(old_inv_path);(D/'original/source-inverses-186854.json').write_bytes(old);inv=json.loads(old)
for row in inv['owners']:
 current=(R/row['source']).read_bytes()
 if sha(current)!=row['current_sha256']:
  assert row['source']==owners[-1]
  previous=blob(row['source']);assert sha(previous)==row['current_sha256']
  row['edits'].insert(0,{'new':current.decode(),'before':previous.decode(),'owner':'docs/evidence/record-copy-field-update-836-backend-controls/source-inverses.json'})
  row['current_sha256']=sha(current)
for row in inv['children']:
 current=(R/row['source']).read_bytes()
 if sha(current)!=row['sha256']:
  previous=blob(row['source']);assert sha(previous)==row['sha256'];ar=D/'original'/row['source'].replace('/','__');assert ar.read_bytes()==previous
  row.setdefault('prior_bindings',[]).append({'sha256':row['sha256'],'archive':str(ar.relative_to(R)),'owner':'docs/evidence/record-copy-field-update-836-backend-controls/source-inverses.json'})
  row['sha256']=sha(current)
(OLD/'source-inverses.json').write_text(json.dumps(inv,indent=2)+'\n')
P=Path('/Volumes/Semaprax/provenance/qualification-registration-module-path-20261014.py');n={};exec(P.read_text().split("parse(R/'src/lib.rs'")[0],n);n['R']=R
for h in ['cleanup_backends','owned_data']:n['parse'](R/('tests/'+h+'.rs'),[],[],'test:'+h,'semaprax')
prior=blob(str(OLD.relative_to(R)/'pending-selectors.json'));(D/'original/pending-selectors-186854.json').write_bytes(prior);old_pending=json.loads(prior)
for row in old_pending['checks']:
 v=n['IDX'][(row['package'],row['harness'],row['selector'])];assert not v['ignored'];row['source_binding']=v;row['source_sha256']=sha((R/row['source']).read_bytes())
(OLD/'pending-selectors.json').write_text(json.dumps(old_pending,indent=2)+'\n')
keys=[k for k in n['IDX']if k[2] in ['owned_leaf_collections::copy_field_update::boundary::exact_native_float_payloads_unrelated_copy_words_and_hostile_carriers_preserve_precommit_and_drop','owned_leaf_vec::copy_field_update::wasm_exact_float_payload_matrix_preserves_every_unrelated_copy_word_and_complete_drop','owned_leaf_vec::copy_field_update::same_source_interpreter_matches_every_scalar_multi_copy_legacy_and_failure_control']];assert len(keys)==3
checks=[]
for k in keys:
 v=n['IDX'][k];assert not v['ignored'];checks.append({'package':k[0],'harness':k[1],'selector':k[2],'source':v['source_path'],'source_sha256':sha((R/v['source_path']).read_bytes()),'source_binding':v,'role':'new','status':'SOURCE_AUTHORED_UNEXECUTED'})
for row in old_pending['checks']:checks.append({**row,'role':'required_existing'})
(D/'pending-selectors.json').write_text(json.dumps({'basis':B,'status':'SOURCE_AUTHORED_UNEXECUTED','new':3,'required_existing':10,'original_five_owning_tests_retained':True,'checks':checks},indent=2)+'\n')
print('SOURCE ONLY coverage4wholeinverses+2children; old13originals+6bindings preserved/rebound;3new+10retained exactbodies')
