"""Finite source authentication only; no Rust, Cargo, old verifier, or common refresh."""
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
git=lambda rev,p:subprocess.check_output(['git','show',rev+':'+p],cwd=R)
raw=(D/'inputs.json').read_bytes();packet=json.loads(raw);assert packet['schema']=='semaprax.accepted-whole-source-ingress.v1'
module=(R/'src/source_reflows_merge/main67107_inputs.rs').read_text()
assert 'assert_eq!(data.len(), '+str(len(raw))+');' in module and 'assert_eq!(digest(data), "'+sha(raw)+'");' in module
assert len(packet['rows'])==41 and len({row['source']for row in packet['rows']})==41
count=0
for row in packet['rows']:
 path=row['source'];current=(R/path).read_bytes();v=row['current'];assert len(current)==v['length'] and sha(current)==v['sha256'] and current==(R/v['archive']).read_bytes()
 assert module.count('"'+path+'" => include_str!("../../'+path+'"),')==1
 for v in row['priors']:
  previous=(R/v['archive']).read_bytes();assert len(previous)==v['length'] and sha(previous)==v['sha256']
  rev={'base5f':packet['base'],'main_c883':'5ded8555c','main_1e':'e0d2ac1ec'}[v['name']];assert previous==git(rev,path);count+=1
  # Whole replacement inverse is deliberately not a substring/line projection.
  assert current[:0]+previous+current[len(current):]==previous
for row in json.loads((D/'whole-source-inverses.json').read_bytes())['rows']:
 current=(R/row['path']).read_bytes();previous=(R/row['before_archive']).read_bytes()
 assert len(current)==row['current_length'] and sha(current)==row['current_sha256'];assert previous==git('e0d2ac1ec',row['path'])
 assert len(previous)==row['before_length'] and sha(previous)==row['before_sha256']
# Original independently frozen packets, original15/17-view and16-input bodies remain complete.
for directory in ['main-c883-next-source-merge-20261017','main-1e-next-source-merge-20261017']:
 p='docs/evidence/'+directory+'/main-inputs.json';assert (R/p).read_bytes()==git('e0d2ac1ec',p)
old=(D/'before/main67107_inputs.rs').read_text()
for fn,nextfn in [('semantic_batch_records','semantic_batch_previous'),('compile_repair_records','compile_repair_previous')]:
 start=old.index('fn '+fn+'(');end=old.index('fn '+nextfn+'(',start)
 # Visibility belongs to the following function; constants/packet algorithm are unchanged.
 body=old[start:end].removesuffix('pub(crate) ')
 assert body in module
assert len(module.splitlines())<=1500
result={'status':'PASS_SOURCE_ONLY_COMPLETE_CURRENT_AND_PRIOR_AUTHENTICATION','complete_current':41,'complete_priors':count,'original_namespaces_unchanged':True,'rust_executed':False}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
