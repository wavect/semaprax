"""Prepare unchanged six bounded drivers and exact five source-bound checks; never execute."""
from pathlib import Path
import argparse, json, ast, shutil, copy, re
from pin_only_contract import SELECTORS, DRIVERS, sha, git, checked_head, prior_qualification, scope

p=argparse.ArgumentParser()
for name in ['registration','out','prior-round']:p.add_argument('--'+name,type=Path,required=True)
for name in ['prior-head','expected-build-receipt-sha256','expected-clippy-receipt-sha256','freeze-head']:p.add_argument('--'+name,required=True)
a=p.parse_args();reg=json.loads(a.registration.read_text());R=Path(reg['repository']).resolve();O=a.out.resolve();assert not O.exists() and not O.is_relative_to(R)
assert not O.is_relative_to(a.prior_round.resolve()) and not O.is_relative_to(Path(reg['pin_evidence']['directory']))
H=a.freeze_head;T=checked_head(R,H);assert reg['source_head']==H and reg['source_tree']==T
assert reg['selected']==reg['resolved']==len(reg['rows'])==7 and not reg['issues']
assert [(r['package'],r['kind'],r['selector'])for r in reg['rows']]==SELECTORS
prior=prior_qualification(R,a.prior_head,a.prior_round,a.expected_build_receipt_sha256,a.expected_clippy_receipt_sha256)
assert prior==reg['prior_qualification'], 'prior receipt drift since registration'
assert scope(R,a.prior_head,H,reg['allow_source_custody_hooks'])==reg['pin_only_source_diff']
for records in [reg['inputs_sha256'],prior['inputs_sha256'],reg['pin_evidence']['inputs_sha256']]:
 for path,digest in records.items():assert sha(path)==digest, ('input drift',path)
checks=[];source={}
for r in reg['rows']:
 pkg=r['package'];kind=r['kind'];target=['--lib'] if kind=='lib' else ['--test',kind[5:]]
 binding={k:copy.deepcopy(r[k]) for k in ['source_path','line','name','attributes','registration_chain','sha256','ignored']}
 assert not binding['ignored'] and sha(R/binding['source_path'])==binding['sha256'];source[binding['source_path']]=binding['sha256']
 for link in binding['registration_chain']:
  link['sha256']=sha(R/link['source_path']);assert link['sha256']==r['registration_chain_sha256'][link['source_path']];source[link['source_path']]=link['sha256']
 q={'package':pkg,'harness':kind,'selector':r['selector'],'command':['cargo','test','--locked','--offline','-p',pkg,*target,r['selector'],'--','--exact','--nocapture','--test-threads=1'],'source_binding':binding,'selected_bodies':[binding],'ignored_bodies_not_executed':[],'expected_selected':1,'status':'UNEXECUTED_SOURCE_AUTHORED','reasons':r['current_batch_selection_reasons']}
 for field in ['prior_actual_failure','prior_actual_observation']:
  if field in r:q[field]=copy.deepcopy(r[field])
 checks.append(q)
for change in reg['pin_only_source_diff']:
 if (R/change['path']).is_file():source[change['path']]=sha(R/change['path'])
source['Cargo.lock']=sha(R/'Cargo.lock')
base=a.prior_round.resolve()/'drivers'
from build_order_contract import original_driver_paths
driver_paths=original_driver_paths(base,json.loads((a.prior_round/'expected-counts.json').read_text()))
# Use the exact six proven prior drivers, with their artifact, source/tree/manifest,
# drift, binary-discovery, ignored/extrabody and resource checks unchanged.
drivers={p.name:sha(p)for p in driver_paths}
prior_manifest=json.loads((a.prior_round/'expected-counts.json').read_text());assert drivers==prior_manifest['prepared_driver_sha256']
for path in driver_paths:ast.parse(path.read_text())
inputs={str(a.registration.resolve()):sha(a.registration),str(Path(__file__).resolve()):sha(__file__),**reg['inputs_sha256'],**prior['inputs_sha256']}
m={'schema':'semaprax.consolidated-successor.v1','status':'FROZEN_BOUND_UNEXECUTED_ROOT_AUTHORIZED','freeze_required':False,'source_subject':H,'source_tree':T,'draft_observed_source_subject':H,'draft_observed_source_tree':T,'draft_source_status':'','draft_repository':str(R),'expected_groups_authored':7,'expected_bodies_authored':7,'checks':checks,'source_file_sha256':source,'source_registration_problems':[],'inputs_sha256':inputs,'prepared_driver_sha256':drivers,'execution_performed':False,'discovery_performed':False,'original_failures_retained':prior['all_prior_actual_failure_count'],'prior_actual_selected_rows':prior['prior_selected_rows'],'prior_actual_executed_bodies':prior['prior_executed_bodies'],'prior_round_source':a.prior_head,'prior_round':str(a.prior_round.resolve()),'prior_qualification':prior,'prior_failed_compilation_attempts':[],'compile_only_harnesses':[],'pending_private_patches':[],'separate_regeneration':[],'environment':copy.deepcopy(prior_manifest['environment']),'scope_note':'Seven reviewed test consumers: three pin consumers, two full source owners, cached observer and help. Prior 145 actual outcomes remain historical; unselected failures are not retested. Production execution source unchanged since explicit prior-head; only approved cfg(test) Rust pin/hook owners and docs/proof metadata may differ. Prior failure count is actual matching executed bodies, possibly zero. No full suite or unrelated reruns.','root_freeze_authorization':'Explicit root authorization for '+H,'pin_only_source_diff':reg['pin_only_source_diff'],'pin_evidence_index':'pin-actual-producer-wire-inverse-evidence.json'}
assert checked_head(R,H)==T
O.mkdir();(O/'drivers').mkdir()
for path in driver_paths:
 shutil.copyfile(path,O/'drivers'/path.name);assert sha(O/'drivers'/path.name)==drivers[path.name]
body=json.dumps(m,indent=2)+'\n';(O/'expected-counts.json').write_text(body);(O/'compilation-manifest.json').write_text(body)
(O/'pin-actual-producer-wire-inverse-evidence.json').write_text(json.dumps(reg['pin_evidence'],indent=2)+'\n')
# Pure command planner only; no subprocess/build/runtime invocation.
node=ast.parse((O/'drivers/build-owning.py').read_text());fn=next(x for x in node.body if isinstance(x,ast.FunctionDef) and x.name=='planned_commands');ns={'re':re};exec(compile(ast.Module(body=[fn],type_ignores=[]),'pure-planner','exec'),ns)
commands=ns['planned_commands'](m,['/Users/kevin/.cargo/bin/rustup','run','1.97.1'])
receipt={'source_subject':H,'source_tree':T,'frozen':True,'source_only':True,'execution_performed':False,'selected_bodies':7,'previous_actual_failed_owners_retained':prior['prior_failed_bodies'],'prior_actual_selected_rows':prior['prior_selected_rows'],'prior_round_source':a.prior_head,'prior_round':str(a.prior_round.resolve()),'prior_build_receipt_sha256':a.expected_build_receipt_sha256,'prior_production_clippy_receipt_sha256':a.expected_clippy_receipt_sha256,'ordered_build_commands':commands,'manifest_sha256':sha(O/'expected-counts.json'),'driver_sha256':drivers,'pin_evidence_index_sha256':sha(O/'pin-actual-producer-wire-inverse-evidence.json'),'scope':'Prepared only. Parent separately schedules source-bound build and seven exact owners; prior production Clippy is reused only under the reviewed production-equivalence scope; no updater/producer/runtime is executed by preparation.'}
(O/'preparation-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');assert checked_head(R,H)==T
print(json.dumps(receipt,indent=2))
