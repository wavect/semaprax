#!/usr/bin/env python3
"""Portable cheap integrity audit; no compiler or test execution."""
import argparse,hashlib,json,re
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--packet',type=Path,default=Path(__file__).resolve().parent);a=p.parse_args();root=a.packet.resolve()
def file(path):
 q=(root/path).resolve();assert q.is_relative_to(root) and q.is_file(),path;return q
sha=lambda path:hashlib.sha256(file(path).read_bytes()).hexdigest()
manifest=json.loads(file('manifest.json').read_text());paths=[]
for row in manifest['assets']:
 paths.append(row['path']);assert file(row['path']).stat().st_size==row['bytes'];assert sha(row['path'])==row['sha256'],row['path']
assert len(paths)==len(set(paths));assert set(paths)=={str(x.relative_to(root)) for x in root.rglob('*') if x.is_file() and x.name!='manifest.json'}
idx=json.loads(file('portable-index.json').read_text());plan=json.loads(file(idx['plan']).read_text());results=json.loads(file(idx['raw_results']).read_text());assert results['complete'] and len(results['results'])==6 and len(idx['owners'])==6
assert results['artifact_source_head']==plan['artifact_source_head']=='83ca0e105e5f4eb6336ed04ff80df6ad0366e99f'
assert results['cwd_source_head']==plan['cwd_source_head']=='0c942afa4004da34ec84639b3f0a2f6e97b52bb8'
assert not results['current0c97_PASS'] and results['original97_status']=='UNEXECUTED_RESOURCE_CAP_ATTEMPTS_NOT_PROMOTED'
assert sha(idx['plan'])==results['plan_sha256'];assert sha('run-six.py')==results['driver_sha256']
assert sha(idx['producer_receipt'])==plan['build_receipt_sha256'];assert sha(idx['producer_metadata'])==plan['build_metadata_sha256'];assert sha(idx['producer_manifest'])==plan['producer_manifest_sha256']
build=json.loads(file(idx['producer_receipt']).read_text());assert build['exit_code']==0 and build['source_subject']==plan['artifact_source_head'];assert len(build['results'])==3
for x in build['results']:assert x['executed'] and x['exit_code']==0 and not x['resource_refusal'] and not x['observation_errors'] and x['group_settlement']['confirmed_empty']
assert build['owning_build_metadata_sha256']==sha(idx['producer_metadata'])
ready=json.loads(file(idx['producer_readiness']).read_text());assert ready['status']=='ACTUAL_PRODUCER_ONLY_THREE_BUILD_ARTIFACTS_VALIDATED_NO_CAPTURE' and ready['source_subject']==plan['artifact_source_head']
assert ready['actual_build_receipt_sha256']==sha(idx['producer_receipt']) and ready['manifest_sha256']==sha(idx['producer_manifest']) and ready['raw_Cargo_log_sha256']==sha(idx['producer_metadata'])
lib=ready['producer_harnesses']['semaprax'];assert lib['executable_sha256']==plan['executable_sha256'] and lib['source_head']==plan['artifact_source_head'] and lib['completed_build_receipt_sha256']==sha(idx['producer_receipt']) and lib['profile_test']
assert lib['profile']=={'opt_level':'0','debuginfo':0,'debug_assertions':True,'overflow_checks':True,'test':True}
assert any(x['executable_sha256']==plan['executable_sha256']=='236a3da674ae2a021c37fd45d6d5fa4f040713fe8af2fae461d91a489c12dd68' and x['target']['kind']==['lib'] for x in build['artifacts'])
for f,s in plan['drivers'].items():assert sha('producer83ca/drivers/'+f)==s
assert sha(idx['production_equivalence'])==plan['production_equivalence_receipt_sha256'];eq=json.loads(file(idx['production_equivalence']).read_text());assert eq['status']=='COMPLETE_PRODUCTION_SOURCE_EQUIVALENCE_PASS' and eq['current_source_head']==plan['cwd_source_head'] and eq['production_source_head']==plan['artifact_source_head']
for x in plan['unchanged_dependency_files']:assert sha(idx['dependencies_prefix']+'/'+x['path'])==x['sha256']
assert len(plan['unchanged_dependency_files'])==53
for r,o,c in zip(results['results'],idx['owners'],plan['checks']):
 assert r['selector']==o['selector']==c['selector'];assert r['valid_PASS'] and r['executed'] and r['exit_code']==0 and not r['resource_refusal'] and not r['observation_errors'] and r['group_settlement']['confirmed_empty'] and r['source_binding_status']=='unchanged'
 assert r['summary']==['ok','1','0','0'] and r['expected_selected_static']==1 and r['discovered_names']==[r['selector']] and not r['current0c97_PASS'];assert r['executable_sha256']==plan['executable_sha256']
 assert sha(o['log'])==r['log_sha256'] and sha(o['memory'])==r['memory_samples_sha256']
 summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',file(o['log']).read_text());assert summaries[-1]==('ok','1','0','0')
 discovery=json.loads(file(o['discovery']).read_text());assert discovery['exit_code']==0 and [x[:-6] for x in discovery['stdout'].splitlines() if x.endswith(': test')]==[r['selector']]
print(json.dumps({'status':'PASS_HISTORICAL_83CA_851_PACKET','passed':6,'failed':0,'assets':len(paths),'unchanged_dependencies':53,'current0c97_PASS':False},sort_keys=True))
