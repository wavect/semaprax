"""Verify #780 publication bytes and require one actual source-bound warm PASS.
No compiler, test process, Git mutation or GitHub operation is performed.
"""
from pathlib import Path
import argparse,hashlib,json,re,shutil,subprocess,sys
P=argparse.ArgumentParser();P.add_argument('--bundle',type=Path,default=Path(__file__).resolve().parent);P.add_argument('--warm-results',type=Path);P.add_argument('--expected-source');P.add_argument('--repo',type=Path);A=P.parse_args();B=A.bundle.resolve();M=json.loads((B/'publication-manifest.json').read_text())
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
assets={x['path']:x for x in M['assets']}
for name,x in assets.items():assert sha(B/name)==x['sha256'],name
assert len(M['original_history'])==148
assert sum(x['exit_code']==0 for x in M['original_history'])==127
assert sum(x['exit_code']!=0 for x in M['original_history'])==21
for x in M['original_history']:
 d=json.loads((B/x['receipt_asset']).read_text());r=next(z for z in d['results'] if z.get('selector')==x['selector']);assert d['source_subject']==x['source'] and r['exit_code']==x['exit_code'];assert sha(B/x['log_asset'])==r['log_sha256']==x['log_sha256']
if A.warm_results is None:
 print('PENDING: historical assets verified; no actual current warm-cache PASS supplied.');sys.exit(2)
assert A.expected_source and re.fullmatch('[0-9a-f]{40}',A.expected_source),'exact final source required'
assert A.repo,'source checkout required for owning cut and fixture binding'
assert A.expected_source==M['prepared_pending_round']['source_subject'], 'prepared frozen subject drift requires explicit new binding'
D=json.loads(A.warm_results.read_text());S=M['required_actual_owner']['selector'];assert D['source_subject']==A.expected_source
matches=[r for r in D['results'] if r.get('selector')==S];assert len(matches)==1;R=matches[0]
assert R['source_subject']==A.expected_source and R['harness']=='semaprax::project'
assert R['executed'] is True and R['exit_code']==0 and not R['timed_out'] and R['resource_refusal'] is None
assert R['summary']==['ok','1','0','0'] and R['discovered_names']==[S]
assert R['source_binding_status']=='unchanged' and not R['observation_errors']
G=R['group_settlement'];assert G['confirmed_empty'] is True and not G['operation_failures'] and not G['observation_errors'] and G['resource_refusal'] is None
assert R['executable_sha256_before']==R['executable_sha256']==R['executable_sha256_after']
log=Path(R['log']);assert sha(log)==R['log_sha256']
assert re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;',log.read_text())
round_=A.warm_results.parent;build=round_/'owning-build-result.json';counts=round_/'expected-counts.json';assert sha(build)==R['owning_build_receipt_sha256'];assert sha(counts)==R['runtime_manifest_sha256']==M['prepared_pending_round']['expected_counts_sha256']
BD=json.loads(build.read_text());CD=json.loads(counts.read_text());assert BD['source_subject']==CD['source_subject']==A.expected_source and BD['exit_code']==0
assert Path(CD['draft_repository']).resolve()==A.repo.resolve(), 'compile-time source checkout must match runtime fixture checkout'
assert BD['owning_build_metadata_sha256']==R['owning_build_metadata_sha256']
buildlog=round_/'owning-build.log';assert sha(buildlog)==BD['log_sha256']
# The finite compiler-feature cut stays exact. Three documented audit/history
# surfaces may contain later additive source ownership; preserve their full diff.
cut=next(x for x in M['assets'] if x['original_path'].endswith('/source-cut-3d4b-to-9ccc.json'));C=json.loads((B/cut['path']).read_text());permitted={'CHANGELOG.md','docs/ARCHITECTURE.md','tests/hir_module_boundaries.rs'}
def git(*a):return subprocess.check_output(['git',*a],cwd=A.repo)
bridge=[]
for x in C['rows']:
 raw=git('show',A.expected_source+':'+x['path']);digest=hashlib.sha256(raw).hexdigest();equal=digest==x['current_sha256'];assert equal or x['path'] in permitted,('unreviewed feature-cut change',x['path'])
 bridge.append({'path':x['path'],'audited_9ccc_sha256':x['current_sha256'],'actual_subject_sha256':digest,'equal':equal})
fixture_paths=['semaprax.toml','src/app.spx','src/frame.spx','src/tests.spx'];fixtures=[]
for p in fixture_paths:
 p='examples/frame-payload-project/'+p;raw=git('show',A.expected_source+':'+p);assert raw==(A.repo/p).read_bytes(),('runtime fixture drift',p);fixtures.append({'path':p,'sha256':hashlib.sha256(raw).hexdigest()})
O=B/'actual';O.mkdir(exist_ok=True);actual_assets=[]
for p in [A.warm_results,log,build,counts,buildlog]:
 dest=O/p.name;shutil.copyfile(p,dest);assert sha(dest)==sha(p);actual_assets.append({'path':str(dest.relative_to(B)),'sha256':sha(dest),'original_path':str(p)})
(O/'source-bridge.diff').write_bytes(git('diff',C['current'],A.expected_source,'--',*sorted(permitted)))
result={'status':'ACTUAL_REQUIRED_WARM_OWNER_PASS_VERIFIED_FOR_PARENT_ACCEPTANCE_REVIEW','source_subject':A.expected_source,'selector':S,'summary':R['summary'],'actual_assets':actual_assets,'owning_source_cut':bridge,'runtime_repo_fixture_binding':fixtures,'historical_rows':148,'historical_original_PASS':127,'historical_original_FAIL_retained':21,'whole_current_head_qualified':False,'positive_application_fit_or_performance_claim':False,'GitHub_mutation':False}
(O/'acceptance-binding.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'status':result['status'],'source_subject':A.expected_source,'selector':S}))
