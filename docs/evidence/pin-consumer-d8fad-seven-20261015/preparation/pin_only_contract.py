"""Read-only pin-only provenance contract. No compiler, updater, or runtime calls."""
from pathlib import Path
import hashlib, json, re, subprocess

SELECTORS = [
 ('semaprax','lib','workspace_graph::identity_resolution::base::cache_billing::tests::cached_prelude_replay_keeps_foreign_core_refusal_sticky'),
 ('semaprax','test:offline_package','resolver_cli::help_keeps_frozen_package_resolve_usage_and_current_cli_snapshot'),
 ('semaprax','lib','workspace_graph::type_proof_tests::projection_tests::rendered_document_has_literal_sha_exact_wire_order_and_digest_binding'),
 ('semaprax','test:workspace','semantic_graph::public_api_cli_bytes_getters_and_read_only_locking_are_exact'),
 ('semaprax','test:workspace','semantic_graph::public_workspace_analysis_api_cli_kats_and_locking_are_exact'),
 ('semaprax','lib','cleanup_plan::replay::live_oracle::source_custody::a::complete_original_sources_and_compiled_owning_receivers_remain_exact'),
 ('semaprax','lib','cleanup_plan::replay::live_oracle::source_custody::b::complete_original_sources_and_compiled_owning_receivers_remain_exact'),
]
DRIVERS = {'build-owning.py','draft_contract.py','observed_phase.py','run-owning-selected.py','run-production-lib-cli-clippy.py','semaprax_pressure_observation_v3.py'}
PIN_RUST = {'src/workspace_graph/type_proof_tests.rs','tests/workspace/semantic_graph.rs'}
HOOK_RUST = {'src/cleanup_plan/replay/live_oracle/source_custody/a.rs','src/cleanup_plan/replay/live_oracle/source_custody/b.rs'}

def sha(path):
 return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def git(repo,*args):
 return subprocess.check_output(['git',*args],cwd=repo,text=True).strip()

def checked_head(repo, expected):
 assert re.fullmatch('[0-9a-f]{40}',expected), 'full approved HEAD required'
 assert git(repo,'rev-parse','HEAD') == expected
 assert not git(repo,'status','--porcelain','--untracked-files=all'), 'clean source including untracked required'
 return git(repo,'rev-parse','HEAD^{tree}')

def bound_json(path, inputs):
 path=Path(path).resolve(); inputs[str(path)]=sha(path)
 return json.loads(path.read_text())

def prior_qualification(repo, head, round_path, build_sha, clippy_sha):
 assert re.fullmatch('[0-9a-f]{40}',head)
 for digest in (build_sha,clippy_sha): assert re.fullmatch('[0-9a-f]{64}',digest)
 rd=Path(round_path).resolve(); inputs={}
 manifest=bound_json(rd/'expected-counts.json',inputs)
 assert (rd/'compilation-manifest.json').read_bytes()==(rd/'expected-counts.json').read_bytes()
 inputs[str(rd/'compilation-manifest.json')]=sha(rd/'compilation-manifest.json')
 tree=git(repo,'rev-parse',head+'^{tree}')
 assert manifest['source_subject']==head and manifest['source_tree']==tree and not manifest['freeze_required']
 assert not manifest['source_registration_problems']
 build=bound_json(rd/'owning-build-result.json',inputs)
 clippy=bound_json(rd/'clippy-production-result.json',inputs)
 assert sha(rd/'owning-build-result.json')==build_sha and sha(rd/'clippy-production-result.json')==clippy_sha
 for rec in (build,clippy):
  assert rec['source_subject']==head and rec['source_tree']==tree and rec['exit_code']==0
  assert rec['manifest_sha256']==sha(rd/'expected-counts.json')
  assert not rec.get('resource_refusal') and not rec.get('artifact_refusals')
 assert build['results'] and all(r['exit_code']==0 and r.get('executed',True) and not r.get('resource_refusal') for r in build['results'])
 assert len(build['ordered_commands'])==len(build['results'])
 assert [r['command'] for r in build['results']]==build['ordered_commands'], 'result/ordered-command mismatch'
 # Locate the actual successful production lib phase by its command, never by
 # phase index. Compatible with unit-first or production-first planners.
 production=[]
 for command,result in zip(build['ordered_commands'],build['results']):
  if 'cargo' not in command:continue
  tail=command[command.index('cargo')+1:]
  if tail and tail[0]=='build' and '-p' in tail and tail[tail.index('-p')+1]=='semaprax' and '--lib' in tail:
   assert '--locked' in tail and '--offline' in tail and '--jobs' in tail and tail[tail.index('--jobs')+1]=='1'
   assert not any(flag in tail for flag in ['--test','--tests','--bin','--bins','--all-targets','--examples','--workspace','--all'])
   production.append(result)
 assert len(production)==1 and production[0]['exit_code']==0, 'one actual successful production lib phase required'
 assert '--lib' in clippy['command'] and '--bin' in clippy['command'] and 'semaprax' in clippy['command'] and 'clippy' in clippy['command']
 assert '-D' in clippy['command'] and 'warnings' in clippy['command']
 assert clippy['command'][clippy['command'].index('-p')+1]=='semaprax'
 assert clippy['command'][clippy['command'].index('--bin')+1]=='semaprax'
 assert not any(flag in clippy['command'] for flag in ['--test','--tests','--all-targets','--examples','--workspace','--all'])
 bindings=[(rd/'owning-build.jsonl',build['owning_build_metadata_sha256']), (rd/'owning-build.log',build['log_sha256']), (rd/'memory-samples.jsonl',build['memory_samples_sha256']), (Path(clippy['log']),clippy['log_sha256']), (Path(clippy['memory_samples_path']),clippy['memory_samples_sha256'])]
 for path,digest in bindings:
  assert sha(path)==digest, ('prior receipt byte drift',str(path)); inputs[str(path.resolve())]=digest
 assert set(manifest['prepared_driver_sha256'])==DRIVERS
 for name,digest in manifest['prepared_driver_sha256'].items():
  assert sha(rd/'drivers'/name)==digest;inputs[str(rd/'drivers'/name)]=digest
 from build_order_contract import accept_build_driver
 accept_build_driver(rd,manifest,build,inputs)
 assert clippy['driver_sha256']==manifest['prepared_driver_sha256']['run-production-lib-cli-clippy.py']
 for path,digest in manifest['source_file_sha256'].items():
  raw=subprocess.check_output(['git','show',head+':'+path],cwd=repo)
  assert hashlib.sha256(raw).hexdigest()==digest, ('prior immutable source binding',path)
 artifacts={str(Path(r['executable']).resolve()):r['executable_sha256'] for r in build['artifacts']}
 assert artifacts
 for path,digest in artifacts.items():
  assert Path(path).resolve().is_relative_to(Path(build['private_target_dir']).resolve())
  assert sha(path)==digest, ('prior artifact drift',path)
 actual=bound_json(rd/'owning-selected-results.json',inputs)
 assert actual['source_subject']==head and actual['complete'] and not actual.get('final_binding_refusal')
 assert len(actual['results'])==len(manifest['checks'])
 expected={(r['package']+'::'+('lib' if r['harness']=='lib' else r['harness'].split(':',1)[1]),r['selector']):r for r in manifest['checks']}
 assert len(expected)==len(manifest['checks'])
 seen=set(); matching=[]
 wanted={(pkg+'::'+('lib' if kind=='lib' else kind.split(':',1)[1]),sel) for pkg,kind,sel in SELECTORS}
 for row in actual['results']:
  key=(row['harness'],row['selector']);assert key in expected and key not in seen;seen.add(key)
  assert row['source_subject']==head and row['runtime_manifest_sha256']==sha(rd/'expected-counts.json')
  assert row['owning_build_receipt_sha256']==build_sha and row['owning_build_metadata_sha256']==build['owning_build_metadata_sha256']
  assert row['driver_sha256']==manifest['prepared_driver_sha256']['run-owning-selected.py']
  assert row['discovered_names']==[row['selector']] and row['expected_selected_static']==expected[key]['expected_selected']==1
  assert not expected[key].get('ignored_bodies_not_executed') and len(expected[key]['selected_bodies'])==1
  if row.get('summary'):
   assert int(row['summary'][3])==0 and int(row['summary'][1])+int(row['summary'][2])<=1
  assert sha(row['log'])==row['log_sha256'];inputs[str(Path(row['log']).resolve())]=row['log_sha256']
  assert artifacts[str(Path(row['executable']).resolve())]==row['executable_sha256']==row['executable_sha256_before']==row['executable_sha256_after']
  if key in wanted:matching.append(row)
 assert seen==set(expected)
 return {'source_subject':head,'source_tree':tree,'round':str(rd),'build_receipt_sha256':build_sha,'production_clippy_receipt_sha256':clippy_sha,'actual_successful_production_phase':production[0],'inputs_sha256':inputs,'matching_prior_actual_rows':matching,'prior_selected_rows':len(matching),'prior_executed_bodies':sum(int(r['summary'][1])+int(r['summary'][2]) for r in matching if r.get('summary')),'prior_failed_bodies':sum(int(r['summary'][2]) for r in matching if r.get('summary')),'all_prior_actual_rows':actual['results'],'all_prior_actual_failed_rows':[r for r in actual['results'] if r.get('summary') and int(r['summary'][2])>0],'all_prior_actual_failure_count':sum(int(r['summary'][2]) for r in actual['results'] if r.get('summary'))}

def scope(repo, prior, current, hooks=False):
 assert prior=='7a87b33115c0cd45b14cac64d7b37e5742503159' and hooks
 checkpoint='1cc913a885fa3b9962b649c4cc4ea1465bc44090'
 assert git(repo,'merge-base','--is-ancestor',prior,current)==''
 expected={line.split('\t')[1] for line in git(repo,'diff','--name-status',prior,checkpoint,'--','*.rs').splitlines() if not line.split('\t')[1].startswith('docs/')}
 assert len(expected)==6
 result=[];seen=set()
 for line in git(repo,'diff','--name-status','--no-renames',prior,current).splitlines():
  status,path=line.split('\t');assert status in {'A','M'}
  mode=git(repo,'ls-tree',current,'--',path).split()[0];assert mode in {'100644','100755'}
  if path.startswith('docs/'):
   assert status=='A', ('old evidence cannot change',path)
   result.append(dict(status=status,path=path,role='additive_metadata_no_authority'));continue
  assert path in expected and status=='M', ('outside reviewed seven-consumer Rust scope',path)
  assert subprocess.check_output(['git','show',current+':'+path],cwd=repo)==subprocess.check_output(['git','show',checkpoint+':'+path],cwd=repo), ('Rust differs from reviewed consumer checkpoint',path)
  seen.add(path);result.append(dict(status=status,path=path,role='exact_reviewed_test_consumer_checkpoint'))
 assert seen==expected
 assert '#[cfg(test)]\nmod source_reflows_merge;' in (repo/'src/lib.rs').read_text()
 return result

def evidence(directory, repo, prior, current, producer_sha, prior_proof, previous_capture, current_capture):
 """Record actual producer/wire/source inverses separately from runtime claims."""
 assert re.fullmatch('[0-9a-f]{64}',producer_sha)
 d=Path(directory).resolve();inputs={}
 old_capture=Path(previous_capture).resolve();new_capture=Path(current_capture).resolve()
 assert old_capture.is_relative_to(d) and new_capture.is_relative_to(d) and old_capture!=new_capture
 record=bound_json(d/'prepared-patch-record.json',inputs)
 assert record['source_basis']==prior and record['later_root_at_preparation']==prior
 for field in ['all10_full_actual_wire_archives_preserved','all10_exact_byte_inverses_verified','complete_previous_pin_capture_archives_unchanged','existing_pin_capture_fullsource_inverse_composition_verified']:assert record[field] is True
 assert record['apply_performed'] is False and record['exclusive_causality_or_gain_claim'] is False
 plan=bound_json(d/'readonly-plan.json',inputs);assert plan['capture_head']==prior
 producer=bound_json(d/'prelude-pin-producers-result.json',inputs)
 assert sha(d/'prelude-pin-producers-result.json')==producer_sha
 assert producer['source_subject']==prior and producer['exit_code']==0 and not producer['resource_refusal']
 assert producer['build_receipt_sha256']==prior_proof['build_receipt_sha256']
 assert plan['trusted_build_receipt_sha256'].removeprefix('sha256:')==prior_proof['build_receipt_sha256']
 assert plan['trusted_producer_receipt_sha256'].removeprefix('sha256:')==producer_sha
 capture_manifest=bound_json(new_capture/'candidates.json',inputs)
 assert capture_manifest['schema']=='semaprax.prelude-forecast-pin-candidates.v1' and not capture_manifest['missing']
 assert capture_manifest['source_head']==(new_capture/'source-head.txt').read_text().strip()==prior
 assert not (new_capture/'source-local.patch').read_bytes()
 assert sha(new_capture/'source-local.patch')==capture_manifest['source_patch_sha256']
 candidates=capture_manifest['candidates'];assert len(candidates)==len({r['name']for r in candidates})==10
 for candidate in candidates:
  wire=(new_capture/candidate['file']).resolve();assert wire.is_relative_to(new_capture)
  assert sha(wire)==candidate['document_sha256'].removeprefix('sha256:') and wire.stat().st_size==candidate['bytes']
 binaries=bound_json(new_capture/'binaries.json',inputs);assert len(binaries)==2
 build=json.loads((d/'owning-build-result.json').read_text());built={str(Path(r['executable']).resolve()):r for r in build['artifacts']}
 command=producer['command'];assert len(command)==5 and command[0]=='/bin/sh'
 assert Path(command[1]).resolve()==repo/'scripts/capture-prelude-forecast-pins.sh'
 assert Path(command[4]).resolve()==Path(producer['capture_out']).resolve()
 for binary,role,executable in zip(binaries,['semaprax','workspace'],command[2:4]):
  key=str(Path(executable).resolve());assert str(Path(binary['path']).resolve())==key and key in built
  assert built[key]['target']['name']==role and built[key]['target']['kind']==(['lib']if role=='semaprax'else['test'])
  assert binary['sha256']==built[key]['executable_sha256']
 assert sha(d/'owning-build-result.json')==prior_proof['build_receipt_sha256']
 for path,digest in record['source_before_sha256'].items():
  assert sha(d/'source-before'/path)==digest
  assert hashlib.sha256(subprocess.check_output(['git','show',prior+':'+path],cwd=repo)).hexdigest()==digest
 for path,digest in record['source_after_sha256'].items():
  assert sha(d/'candidate-source-after'/path)==digest
  # Later additive custody metadata may compose the pin packet; Rust pins must stay exact.
  if path.endswith('.rs'):assert sha(repo/path)==digest
 wires=bound_json(d/'whole-wire-exact-inverses.json',inputs);assert len(wires)==10
 for row in wires:
  name=row['wire']+'.json';before=old_capture/'wires'/name;after=new_capture/'wires'/name
  old=before.read_bytes();new=after.read_bytes();assert hashlib.sha256(old).hexdigest()==row['before_sha256'] and hashlib.sha256(new).hexdigest()==row['after_sha256']
  restored=new
  for e in reversed(row['exact_byte_inverse']):
   assert restored[e['after_start']:e['after_end']]==bytes.fromhex(e['after_hex'])
   restored=restored[:e['after_start']]+bytes.fromhex(e['before_hex'])+restored[e['after_end']:]
  assert restored==old
 # Bind every retained actual capture/report/archive byte; do not fabricate wire values.
 for path in d.rglob('*'):
  assert not path.is_symlink(), 'evidence symlinks are not accepted'
  if path.is_file():inputs[str(path.resolve())]=sha(path)
 return {'directory':str(d),'source_basis':prior,'current_pin_head':current,'actual_producer_receipt_sha256':producer_sha,'source_and_whole_wire_inverses_verified':True,'inputs_sha256':inputs,'runtime_consumer_success_claim':False}
