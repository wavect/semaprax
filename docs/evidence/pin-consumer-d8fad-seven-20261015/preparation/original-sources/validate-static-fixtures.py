"""Finite file/AST provenance guard fixtures only, never prior_qualification/build."""
import copy,json,shutil,tempfile,hashlib
from pathlib import Path
from build_order_contract import accept_build_driver,original_driver_paths,DERIVED_SHA,INSTALLER_SHA,PROOF_SHA,NEW_LABEL
from build_order_adapter_proof import prove,planner
HERE=Path(__file__).resolve().parent
SOURCE=Path('/Volumes/Semaprax/provenance/next-focused-tooling-v2-20261015')
proof=json.loads((SOURCE/'preservation-proof.json').read_text())
fixture=json.loads(Path('/Volumes/Semaprax/provenance/low-ram-build-order-adapter-20261015/fixture230.json').read_text())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
passed=[]
with tempfile.TemporaryDirectory(prefix='pin-order-source-fixture-',dir='/Volumes/Semaprax/provenance') as temporary:
 rd=Path(temporary);(rd/'drivers').mkdir()
 for name in proof['six_original_drivers']:shutil.copyfile(SOURCE/'original-drivers'/name,rd/'drivers'/name)
 m=copy.deepcopy(fixture);m.update(source_subject='a'*40,prepared_driver_sha256=proof['six_original_drivers'],prepared_extra_driver_sha256={'run-extra-production-clippy.py':sha(SOURCE/'run-extra-production-clippy.py')})
 shutil.copyfile(SOURCE/'run-extra-production-clippy.py',rd/'drivers/run-extra-production-clippy.py')
 (rd/'expected-counts.json').write_text(json.dumps(m,indent=2)+'\n')
 original=(rd/'drivers/build-owning.py').read_text();derived=Path('/Volumes/Semaprax/provenance/low-ram-build-order-adapter-20261015/build-owning-low-ram.py').read_text()
 accept_build_driver(rd,m,{'driver_sha256':m['prepared_driver_sha256']['build-owning.py']},{})
 assert len(original_driver_paths(rd/'drivers',m))==6;passed.append('original_exact_driver_with_manifest_bound_extra_clippy')
 (rd/'drivers/build-owning-low-ram.py').write_text(derived)
 adapter={'status':'SEPARATE_ADAPTER_INSTALLED_UNEXECUTED','round':str(rd),'manifest':str(rd/'expected-counts.json'),'manifest_sha256':sha(rd/'expected-counts.json'),'source_subject':m['source_subject'],'all_six_original_drivers_sha256':m['prepared_driver_sha256'],'original_driver_sha256':m['prepared_driver_sha256']['build-owning.py'],'derived_driver_sha256':DERIVED_SHA,'original_driver':str(rd/'drivers/build-owning.py'),'derived_driver':str(rd/'drivers/build-owning-low-ram.py'),'build_target_selection':NEW_LABEL,'parent_required_rss_limit_gib':7,'executed_build_commands':0,'executed_test_bodies':0,'installer_sha256':INSTALLER_SHA,'adapter_support_sha256':PROOF_SHA,'static_proof':prove(original,derived,m)}
 def write_adapter(value): (rd/'build-order-adapter.json').write_text(json.dumps(value,indent=2)+'\n')
 write_adapter(adapter)
 commands=planner(derived)(m,['/Users/kevin/.cargo/bin/rustup','run','1.97.1'])
 build={'driver_sha256':DERIVED_SHA,'build_target_selection':NEW_LABEL,'executed_test_bodies':0,'ordered_commands':commands,'results':[{'command':c} for c in commands]}
 inputs={};accept_build_driver(rd,m,build,inputs);assert len(original_driver_paths(rd/'drivers',m))==6
 assert str((rd/'drivers/build-owning-low-ram.py').resolve()) in inputs;passed.append('derived_full_source_inverse_AST_and_actual_eight_command_order')
 def reject(label,value=None,receipt=None):
  if receipt is not None:write_adapter(receipt)
  try:accept_build_driver(rd,m,build if value is None else value,{})
  except (AssertionError,FileNotFoundError):passed.append('refused_'+label)
  else:raise AssertionError('Unexpected acceptance: '+label)
  write_adapter(adapter)
 bad=copy.deepcopy(build);bad['driver_sha256']='0'*64;reject('unknown_driver_sha',bad)
 bad=copy.deepcopy(build);bad['ordered_commands']=list(reversed(bad['ordered_commands']));bad['results']=[{'command':c} for c in bad['ordered_commands']];reject('wrong_actual_order',bad)
 bad=copy.deepcopy(build);bad['results'][0]['command']=['fake'];reject('actual_result_command_drift',bad)
 bad=copy.deepcopy(adapter);bad['static_proof']['whole_source_inverse_exact']=False;reject('claimed_AST_proof_drift',receipt=bad)
 bad=copy.deepcopy(adapter);bad['parent_required_rss_limit_gib']=9;reject('parent_cap_metadata_relaxation',receipt=bad)
 bad=copy.deepcopy(adapter);bad['manifest_sha256']='0'*64;reject('manifest_hash_drift',receipt=bad)
 p=rd/'drivers/build-owning-low-ram.py';p.write_text(derived+'\n# drift\n');reject('derived_source_drift');p.write_text(derived)
 p=rd/'drivers/unknown.py';p.write_text('');reject('unlisted_extra_driver');p.unlink()
 p=rd/'drivers/draft_contract.py';before=p.read_bytes();p.write_bytes(before+b'\n');reject('original_six_driver_drift');p.write_bytes(before)
 p=rd/'build-order-adapter.json';before=p.read_bytes();p.unlink();reject('missing_adapter_receipt');p.write_bytes(before)
report={'status':'SYNTHETIC_SOURCE_FILE_AND_PURE_AST_GUARD_FIXTURES_ONLY','cases':passed,'positive_cases':2,'negative_cases':10,'prior_qualification_called':False,'build_capture_or_pin_execution':False,'runtime_or_real_receipt_acceptance_claim':False}
(HERE/'static-fixture-proof.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
