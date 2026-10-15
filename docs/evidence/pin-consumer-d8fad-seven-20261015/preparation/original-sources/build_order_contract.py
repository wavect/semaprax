"""Verify the exact minimal order adapter; no driver/build/capture execution."""
import hashlib,json,re
from pathlib import Path
from build_order_adapter_proof import derive,prove,planner,ORIGINAL_SHA,NEW_LABEL
DERIVED_SHA='6f469698ec9aee8d143d27ef2c6136d3780523405c1300e8e8de34965c14d252'
PROOF_SHA='aea6c3e1b70a1ac0da531b1f693cd53b193271d9e1a3b873b3bea98157795164'
INSTALLER_SHA='0461b30fbe11fcd732fece030a78546720bfc1255d34cdf30feb29094d38be45'
DRIVERS={'build-owning.py','draft_contract.py','observed_phase.py','run-owning-selected.py','run-production-lib-cli-clippy.py','semaprax_pressure_observation_v3.py'}
EXTRA='build-owning-low-ram.py'
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def read(path,inputs):
 path=Path(path).resolve();inputs[str(path)]=sha(path);return json.loads(path.read_text())
def inventory(base,manifest,inputs):
 base=Path(base).resolve();assert set(manifest['prepared_driver_sha256'])==DRIVERS
 declared=manifest.get('prepared_extra_driver_sha256',{})
 assert set(declared)<= {'run-extra-production-clippy.py'},'Unknown declared extra driver'
 expected=set(DRIVERS)|set(declared)
 if (base/EXTRA).exists():expected.add(EXTRA)
 assert {p.name for p in base.glob('*.py')}==expected,'Unexpected driver inventory'
 for name,digest in {**manifest['prepared_driver_sha256'],**declared}.items():
  path=base/name;assert not path.is_symlink() and re.fullmatch('[0-9a-f]{64}',digest)
  assert sha(path)==digest;inputs[str(path)]=digest
 return EXTRA in expected

def adapter_proof(rd,manifest,inputs):
 rd=Path(rd).resolve();path=rd/'build-order-adapter.json';receipt=read(path,inputs)
 assert receipt['status']=='SEPARATE_ADAPTER_INSTALLED_UNEXECUTED'
 assert Path(receipt['round']).resolve()==rd
 assert Path(receipt['manifest']).resolve()==rd/'expected-counts.json'
 assert receipt['manifest_sha256']==sha(rd/'expected-counts.json')
 assert receipt['source_subject']==manifest['source_subject']
 assert receipt['all_six_original_drivers_sha256']==manifest['prepared_driver_sha256']
 assert receipt['original_driver_sha256']==manifest['prepared_driver_sha256']['build-owning.py']==ORIGINAL_SHA
 assert receipt['derived_driver_sha256']==DERIVED_SHA
 assert Path(receipt['original_driver']).resolve()==rd/'drivers/build-owning.py'
 derived=rd/'drivers'/EXTRA;assert not derived.is_symlink()
 assert Path(receipt['derived_driver']).resolve()==derived
 assert sha(derived)==DERIVED_SHA
 assert receipt['build_target_selection']==NEW_LABEL and receipt['parent_required_rss_limit_gib']==7
 assert receipt['executed_build_commands']==receipt['executed_test_bodies']==0
 assert receipt['installer_sha256']==INSTALLER_SHA and receipt['adapter_support_sha256']==PROOF_SHA
 support=Path(__file__).with_name('build_order_adapter_proof.py');assert sha(support)==PROOF_SHA
 inputs[str(support.resolve())]=sha(support);inputs[str(derived)]=DERIVED_SHA
 original=(rd/'drivers/build-owning.py').read_text();current=derived.read_text()
 assert current==derive(original),'Full original-to-derived source inverse drift'
 computed=prove(original,current,manifest)
 assert computed['selected_bodies']==230 and computed['literal_roles']==16
 assert computed==receipt['static_proof'],'Actual manifest AST/planner proof differs from installed proof'
 assert computed['whole_source_inverse_exact'] and computed['entire_other_ast_exact_after_only_receipt_label_normalization']
 return current

def accept_build_driver(rd,manifest,build,inputs):
 inputs[str(Path(__file__).resolve())]=sha(__file__)
 rd=Path(rd).resolve();has_adapter=inventory(rd/'drivers',manifest,inputs)
 current=adapter_proof(rd,manifest,inputs) if has_adapter else None
 original=manifest['prepared_driver_sha256']['build-owning.py']
 if build['driver_sha256']==original:
  # Preserve the exact old acceptance domain. Existing contract still binds
  # every actual result/command, production phase, artifacts and runtime rows.
  return
 assert has_adapter and build['driver_sha256']==DERIVED_SHA,'Unverified build driver'
 assert build['build_target_selection']==NEW_LABEL and build['executed_test_bodies']==0
 expected=planner(current)(manifest,['/Users/kevin/.cargo/bin/rustup','run','1.97.1'])
 assert build['ordered_commands']==expected,'Actual commands differ from the exact verified order adapter'
 assert [r['command'] for r in build['results']]==expected,'Actual results do not cover every ordered command'
 # All phases are matched by complete command; no phase index is authority.
 assert len(expected)==8

def original_driver_paths(base,manifest):
 """Verified known extras remain prior evidence; later round copies six originals."""
 base=Path(base).resolve();inputs={};has_adapter=inventory(base,manifest,inputs)
 if has_adapter:adapter_proof(base.parent,manifest,inputs)
 return [base/name for name in sorted(DRIVERS)]
