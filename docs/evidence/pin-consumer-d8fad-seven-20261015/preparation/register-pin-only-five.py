"""Future source-only registration: exact three pin consumers and two source owners."""
import argparse, json, hashlib
from pathlib import Path
from pin_only_contract import SELECTORS, sha, git, checked_head, prior_qualification, scope, evidence

p=argparse.ArgumentParser()
for name in ['repo','out','prior-round','pin-evidence','previous-capture','current-capture']:p.add_argument('--'+name,type=Path,required=True)
for name in ['expected-head','prior-head','expected-build-receipt-sha256','expected-clippy-receipt-sha256','expected-producer-receipt-sha256']:p.add_argument('--'+name,required=True)
p.add_argument('--allow-source-custody-hooks',action='store_true')
p.add_argument('--registration-parser',type=Path,default=Path('/Volumes/Semaprax/provenance/qualification-registration-module-path-20261014.py'))
a=p.parse_args();R=a.repo.resolve();O=a.out.resolve();assert not O.exists() and not O.is_relative_to(R)
assert not O.is_relative_to(a.prior_round.resolve()) and not O.is_relative_to(a.pin_evidence.resolve())
H=a.expected_head;T=checked_head(R,H)
prior=prior_qualification(R,a.prior_head,a.prior_round,a.expected_build_receipt_sha256,a.expected_clippy_receipt_sha256)
changes=scope(R,a.prior_head,H,a.allow_source_custody_hooks)
evs=evidence(a.pin_evidence,R,a.prior_head,H,a.expected_producer_receipt_sha256,prior,a.previous_capture,a.current_capture)
P=a.registration_parser.resolve();prefix=P.read_text().split("parse(R/'src/lib.rs'")[0]
# Preserve the exact registered module walker; fail rather than overwrite duplicate bodies.
class UniqueIndex(dict):
 def __setitem__(self,key,value):
  assert key not in self, ('duplicate registered test identity',key)
  super().__setitem__(key,value)
assert prefix.count('IDX={};')==1
adapted=prefix.replace('IDX={};','IDX=UniqueIndex();',1);n={'UniqueIndex':UniqueIndex};exec(adapted,n);n['R']=R
n['parse'](R/'src/lib.rs',[],[],'lib','semaprax')
n['parse'](R/'tests/workspace.rs',[],[],'test:workspace','semaprax')
n['parse'](R/'tests/offline_package.rs',[],[],'test:offline_package','semaprax')
rows=[];inputs={str(P):sha(P),str(Path(__file__).resolve()):sha(__file__),str(Path(__file__).with_name('pin_only_contract.py').resolve()):sha(Path(__file__).with_name('pin_only_contract.py'))}
for pkg,kind,selector in SELECTORS:
 v=n['IDX'].get((pkg,kind,selector));assert v is not None and not v['ignored']
 if kind=='lib':assert any('#[cfg(test)]' in x['attributes'] for x in v['registration_chain']), ('library pin/source owner must be test-only',selector)
 har='lib' if kind=='lib' else kind.split(':',1)[1]
 row={'package':pkg,'kind':kind,'harness':pkg+'::'+har,'selector':selector,'reason':'Pin-only actual producer literal correction; exact pin consumer or complete original source owner',**v,'binding_repository':str(R),'registration_chain_sha256':{x['source_path']:sha(R/x['source_path']) for x in v['registration_chain']},'current_batch_selection_reasons':['Pin-only three consumers plus two complete source owners; not a claim of prior failure']}
 actual=[x for x in prior['matching_prior_actual_rows'] if (x['harness'],x['selector'])==(row['harness'],selector)]
 assert len(actual)<=1
 if actual:
  row['prior_actual_observation']=actual[0]
  s=actual[0].get('summary')
  if s and int(s[2])>0:
   row['prior_actual_failure']={**actual[0],'receipt':str(a.prior_round.resolve()/'owning-selected-results.json'),'receipt_sha256':sha(a.prior_round/'owning-selected-results.json')}
 rows.append(row)
assert len(rows)==7 and len({(r['package'],r['kind'],r['selector']) for r in rows})==7
# The explicit source-custody opt-in must still resolve only registered test paths.
registered={r['source_path'] for r in rows}|{x['source_path'] for r in rows for x in r['registration_chain']}
for change in changes:
 if change['path'].endswith('.rs') and not change['path'].startswith('docs/'):assert change['path'] in registered or change['path']=='src/source_reflows_merge.rs'
reg={'source_head':H,'source_tree':T,'repository':str(R),'selected':7,'resolved':7,'rows':rows,'issues':[],'inputs_sha256':inputs,'state':'PIN_ONLY_CORRECTION_UNEXECUTED','prior_failed_bodies':prior['prior_failed_bodies'],'prior_selected_rows':prior['prior_selected_rows'],'prior_executed_bodies':prior['prior_executed_bodies'],'prior_qualification':prior,'pin_evidence':evs,'pin_only_source_diff':changes,'allow_source_custody_hooks':a.allow_source_custody_hooks,'registration_parser_sha256':sha(P),'original_prefix_sha256':hashlib.sha256(prefix.encode()).hexdigest(),'adaptation':'Same module walker; duplicate-identity overwrite now fails. Prior failure counts derive only from actual selected receipt rows at explicit prior-head/prior-round. No capture/update/build/runtime performed.'}
assert checked_head(R,H)==T
O.mkdir();(O/'registration.json').write_text(json.dumps(reg,indent=2)+'\n')
(O/'pin-actual-producer-wire-inverse-evidence.json').write_text(json.dumps(evs,indent=2)+'\n')
print(json.dumps({'source':H,'selected':7,'prior_selected_rows':prior['prior_selected_rows'],'prior_failed_bodies':prior['prior_failed_bodies'],'issues':[]}))
