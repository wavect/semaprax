"""Fixed Point metadata transition, not an extension of budget-only admission.
Actual capture and derived compatibility wires are separate authenticated domains.
No producer receipt is ever checked against derived bytes. No build or capture.
"""
from pathlib import Path
import json, hashlib, importlib.util, argparse, shutil, difflib, subprocess
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
enc=lambda x:json.dumps(x,separators=(',',':'),ensure_ascii=False).encode()
spec=importlib.util.spec_from_file_location('unchanged_managed_history',HERE/'unchanged-history-tools/managed_history.py')
history=importlib.util.module_from_spec(spec);spec.loader.exec_module(history)

def digest(data,name):
 key='graph_digest' if name in ('public-graph','projection-wire') else 'artifact_digest'
 domain=(b'semaprax.workspace-semantic-graph.artifact-digest.v1\0' if key=='graph_digest' else ('semaprax.workspace-semantic-'+('review' if name.endswith('review') else 'impact' if name.endswith('impact') else 'context')+'.artifact-digest.v1\0').encode())
 raw=enc(data);field=b','+enc(key)+b':'+enc(data[key]);assert raw.count(field)==1
 payload=raw.replace(field,b'',1);return 'sha256:'+sha(domain+len(payload).to_bytes(8,'little')+payload)

def authenticate(capture, paths, expected_hashes, head, original_capture):
 for role, path in paths.items():assert sha(path.read_bytes())==expected_hashes[role]
 readiness,build,producer,expected=[json.loads(paths[k].read_bytes()) for k in ['readiness','build','producer','manifest']]
 assert readiness['status']=='ACTUAL_PRODUCER_ONLY_THREE_BUILD_ARTIFACTS_VALIDATED_NO_CAPTURE'
 assert readiness['source_subject']==build['source_subject']==producer['source_subject']==expected['source_subject']==head
 assert build['exit_code']==producer['exit_code']==0 and not producer['resource_refusal']
 assert producer['build_receipt_sha256']==expected_hashes['build']==readiness['actual_build_receipt_sha256']
 assert readiness['manifest_sha256']==expected_hashes['manifest']
 command=producer['command'];assert len(command)==5 and command[0]=='/bin/sh'
 assert Path(command[1]).resolve()==Path(expected['draft_repository']).resolve()/'scripts/capture-prelude-forecast-pins.sh'
 assert Path(command[4]).resolve()==Path(producer['capture_out']).resolve()==original_capture.resolve()
 assert (capture/'source-head.txt').read_text().strip()==head
 candidate=json.loads((capture/'candidates.json').read_bytes())
 assert candidate['schema']=='semaprax.prelude-forecast-pin-candidates.v1' and candidate['source_head']==head and not candidate['missing']
 assert not (capture/'source-local.patch').read_bytes() and candidate['source_patch_sha256']==sha(b'')
 saved=json.loads((capture/'binaries.json').read_bytes());built={r['executable']:r for r in build['artifacts']}
 assert len(saved)==2
 for record,role,executable in zip(saved,['semaprax','workspace'],command[2:4]):
  ready=readiness['producer_harnesses'][role];artifact=built[executable]
  assert record['path']==executable==ready['executable']
  assert record['sha256']==ready['executable_sha256']==artifact['executable_sha256']
  assert artifact['target']['kind']==(['lib'] if role=='semaprax' else ['test']) and artifact['target']['name']==role
 statuses={}
 for line in (capture/'status.tsv').read_text().splitlines():
  label,code,selector=line.split('\t');assert label not in statuses;statuses[label]=(int(code),selector)
 for label,selector in [('public-graph','semantic_graph::public_api_cli_bytes_getters_and_read_only_locking_are_exact'),('analysis','semantic_graph::public_workspace_analysis_api_cli_kats_and_locking_are_exact'),('projection-wire','workspace_graph::type_proof_tests::projection_tests::emit_projection_wire_pin_candidate')]:
  assert statuses[label][1]==selector and statuses[label][0] in (0,101)
  log=(capture/(label+'.log')).read_text();assert selector in log and 'running 1 test' in log
  if statuses[label][0]==0:assert 'test result: ok. 1 passed; 0 failed' in log
 assert statuses['projection-wire'][0]==0
 rows=candidate['candidates'];assert len(rows)==10 and len({r['name'] for r in rows})==10
 for row in rows:
  assert row['file']=='wires/'+row['name']+'.json'
  raw=(capture/row['file']).read_bytes();assert row['document_sha256']=='sha256:'+sha(raw) and row['bytes']==len(raw)
 return {'source_head':head,'receipt_hashes':expected_hashes,'historical_only':original_capture.resolve()!=capture.resolve()}

def nested_digests(value, revision, graph_digest):
 if isinstance(value,list):
  for item in value:nested_digests(item,revision,graph_digest)
 elif isinstance(value,dict):
  if 'workspace_revision' in value:assert value['workspace_revision']==revision
  if 'workspace_graph_digest' in value:assert value['workspace_graph_digest']==graph_digest
  if 'artifact_digest' in value:
   kind=next(k for k in ['context','impact','review'] if value['schema'].startswith('semaprax.workspace-semantic-'+k+'.'))
   assert value['artifact_digest']==digest(value,kind)
  for child in value.values():nested_digests(child,revision,graph_digest)

def manifest(graph):
 assert [(m['path'],m['module']) for m in graph['modules']]==[('a/provider.spx','public.provider'),('z/app.spx','public.app')]
 rows=[dict(path=m['path'],source_graph_schema=m['source_graph_schema'],source_revision=m['source_revision'],source_digest=m['source_digest'],bytes=n) for m,n in zip(graph['modules'],[282,324])]
 assert sum(x['bytes'] for x in rows)==graph['budget']['used_total_source_bytes']==606
 raw=enc(dict(schema='semaprax.workspace-semantic-manifest.v1',files=rows))+b'\n'
 return raw,'sha256:'+sha(b'semaprax.workspace-semantic-revision.v1\0'+len(raw).to_bytes(8,'little')+raw)

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--capture',type=Path)
 parser.add_argument('--expected-head')
 parser.add_argument('--root',type=Path,default=ROOT)
 parser.add_argument('--out',type=Path)
 parser.add_argument('--round',type=Path)
 for role in ['readiness','build-receipt','producer-receipt']:
  parser.add_argument('--'+role,type=Path);parser.add_argument('--'+role+'-sha256')
 args=parser.parse_args()
 if args.capture:
  assert args.out and not args.out.exists() and args.expected_head and args.round
  for key in ['readiness','build_receipt','producer_receipt']:assert getattr(args,key) and getattr(args,key+'_sha256')
  assert args.build_receipt.resolve()==(args.round/'owning-build-result.json').resolve()
  assert args.producer_receipt.resolve()==(args.round/'prelude-pin-producers-result.json').resolve()
  from types import SimpleNamespace
  front_spec=importlib.util.spec_from_file_location('actual_capture_frontier',HERE/'actual_capture_frontier.py')
  front=importlib.util.module_from_spec(front_spec);front_spec.loader.exec_module(front)
  front.validate_actual_capture(SimpleNamespace(capture=args.capture,repo=args.root,round=args.round,expected_capture_head=args.expected_head,expected_build_receipt_sha256=args.build_receipt_sha256,expected_producer_receipt_sha256=args.producer_receipt_sha256))
  readiness,build,producer=[json.loads(getattr(args,k).read_text()) for k in ['readiness','build_receipt','producer_receipt']]
  for key in ['readiness','build_receipt','producer_receipt']:
   assert sha(getattr(args,key).read_bytes())==getattr(args,key+'_sha256')
  assert readiness['status']=='ACTUAL_PRODUCER_ONLY_THREE_BUILD_ARTIFACTS_VALIDATED_NO_CAPTURE'
  assert readiness['source_subject']==build['source_subject']==producer['source_subject']==args.expected_head
  assert build['exit_code']==producer['exit_code']==0 and not producer['resource_refusal']
  assert producer['build_receipt_sha256']==args.build_receipt_sha256==readiness['actual_build_receipt_sha256']
  assert Path(producer['capture_out']).resolve()==args.capture.resolve()
  command=producer['command'];assert len(command)==5 and command[0]=='/bin/sh' and Path(command[4]).resolve()==args.capture.resolve()
  assert Path(command[1]).name=='capture-prelude-forecast-pins.sh'
  assert (args.capture/'source-head.txt').read_text().strip()==args.expected_head
  candidate=json.loads((args.capture/'candidates.json').read_text())
  assert candidate['schema']=='semaprax.prelude-forecast-pin-candidates.v1' and candidate['source_head']==args.expected_head and not candidate['missing']
  assert not (args.capture/'source-local.patch').read_bytes() and candidate['source_patch_sha256']==sha(b'')
  saved=json.loads((args.capture/'binaries.json').read_text());built={r['executable']:r for r in build['artifacts']}
  for record,role,executable in zip(saved,['semaprax','workspace'],command[2:4]):
   ready=readiness['producer_harnesses'][role]
   assert record['path']==executable==ready['executable']
   assert record['sha256']==ready['executable_sha256']==built[executable]['executable_sha256']
  assert len(saved)==2
  for row in candidate['candidates']:
   raw=(args.capture/row['file']).read_bytes()
   assert row['document_sha256']=='sha256:'+sha(raw) and row['bytes']==len(raw)
  args.out.mkdir();shutil.copytree(args.capture,args.out/'actual-capture')
 binding=json.loads((HERE/'transition-binding.json').read_text());actual=args.capture if args.capture else HERE/'actual67b';old=HERE/'actual83ca';out=args.out/'derived-compatibility' if args.capture else HERE/'derived-compatibility';out.mkdir(exist_ok=True)
 (out/'DERIVED.md').write_text('DERIVED compatibility view. These bytes are not producer output, capture evidence, or producer authority. Only the unchanged historical byte-restoration checker consumes this view.\n')
 for name,expected in binding['unchanged_history_tool_sha256'].items():assert sha((HERE/'unchanged-history-tools'/name).read_bytes())==expected
 assert (actual/'source-head.txt').read_text().strip()==(args.expected_head if args.capture else binding['actual_head'])
 assert not (actual/'source-local.patch').read_bytes()
 fixture=(args.root/'tests/workspace/semantic_graph.rs').read_text()
 start=fixture.index('    fn stage(');end=fixture.index('    fn new(',start)
 assert sha(fixture[start:end].encode())==binding['fixture_stage_sha256']
 captured_source=subprocess.check_output(['git','show',(args.expected_head if args.capture else binding['actual_head'])+':tests/workspace/semantic_graph.rs'],cwd=args.root).decode()
 captured_start=captured_source.index('    fn stage(');captured_end=captured_source.index('    fn new(',captured_start)
 assert sha(captured_source[captured_start:captured_end].encode())==binding['fixture_stage_sha256']
 if not args.capture:
  for p,h in binding['actual_files'].items():assert sha((actual/p).read_bytes())==h,p
 authentication_paths={'readiness':args.readiness,'build':args.build_receipt,'producer':args.producer_receipt,'manifest':args.round/'expected-counts.json'} if args.capture else {k:HERE/'actual-authentication'/v for k,v in [('readiness','readiness.json'),('build','owning-build-result.json'),('producer','prelude-pin-producers-result.json'),('manifest','expected-counts.json')]}
 authentication_hashes={k:sha(p.read_bytes()) for k,p in authentication_paths.items()}
 if not args.capture:
  for p,h in binding['authentication_files'].items():assert sha((HERE/'actual-authentication'/p).read_bytes())==h
 authentication=authenticate(actual,authentication_paths,authentication_hashes,args.expected_head if args.capture else binding['actual_head'],args.capture if args.capture else Path(binding['original_capture_path']))
 current={p.name:history.self_digest(p.read_bytes(),p.stem) for p in (actual/'wires').glob('*.json')}
 prior={p.name:history.self_digest(p.read_bytes(),p.stem) for p in (old/'wires').glob('*.json')}
 assert set(current)==set(prior)==set(binding['actual_wire_sha256'])
 a=current['public-graph.json'];b=prior['public-graph.json']
 for m,n in zip(a['modules'],b['modules']):
  assert {k:v for k,v in m.items() if k!='source_graph_schema'}=={k:v for k,v in n.items() if k!='source_graph_schema'}
 assert a['modules'][0]['source_graph_schema']==b['modules'][0]['source_graph_schema']=='semaprax.graph.v10'
 assert a['modules'][1]['source_graph_schema']=='semaprax.graph.v10' and b['modules'][1]['source_graph_schema']=='semaprax.graph.v74'
 new_manifest,new_revision=manifest(a);old_manifest,old_revision=manifest(b)
 assert new_revision==a['workspace_revision']==binding['new_revision']
 assert old_revision==b['workspace_revision']==binding['old_revision']
 for data in current.values():
  if data is not current['projection-wire.json']:nested_digests(data,new_revision,a['graph_digest'])
 for data in prior.values():
  if data is not prior['projection-wire.json']:nested_digests(data,old_revision,b['graph_digest'])
 (out/'new-manifest.json').write_bytes(new_manifest);(out/'old-manifest.json').write_bytes(old_manifest)
 graph=json.loads(enc(a));graph['modules'][1]['source_graph_schema']='semaprax.graph.v74';graph['workspace_revision']=old_revision;graph['graph_digest']=digest(graph,'public-graph')
 graph_before=a['graph_digest'];graph_after=graph['graph_digest']
 def project(data,name):
  if name=='projection-wire':return data
  if name=='public-graph':return graph
  def walk(value):
   if not isinstance(value,dict):return
   for k,v in value.items():
    if k=='workspace_revision':assert v==new_revision;value[k]=old_revision
    elif k=='workspace_graph_digest':assert v==graph_before;value[k]=graph_after
    elif isinstance(v,dict):walk(v)
   if 'artifact_digest' in value:
    kind='review' if 'context' in value and 'impact' in value else 'impact' if 'changes' in value or 'direction' in value else 'context'
    # Root/nested kind is selected by the existing wire schema, not guessed fields.
    schema=value['schema'];assert schema.startswith('semaprax.workspace-semantic-')
    kind=next(k for k in ['context','impact','review'] if schema.startswith('semaprax.workspace-semantic-'+k+'.'))
    value['artifact_digest']=digest(value,kind)
  result=json.loads(enc(data));walk(result);return result
 rows=[]
 for filename,data in current.items():
  projected=project(data,Path(filename).stem)
  if filename!='projection-wire.json':nested_digests(projected,old_revision,graph_after)
  raw=enc(projected);history.self_digest(raw,Path(filename).stem)
  dst=out/'wires'/filename;dst.parent.mkdir(exist_ok=True);dst.write_bytes(raw)
  if not args.capture:assert sha(raw)==binding['derived_wire_sha256'][filename]
  before=(old/'wires'/filename).read_bytes()
  # Whole original checker: its unchanged allowed paths do not contain schema/revision.
  allow=json.loads((HERE/'unchanged-history-tools/managed-allowed-paths.json').read_text())
  history.restore_exact(raw,before,allow[filename])
  rows.append({'wire':filename,'actual_sha256':sha((actual/'wires'/filename).read_bytes()),'derived_compatibility_sha256':sha(raw),'old83ca_sha256':sha(before)})
 # Complete unchanged independent47/83ca/55e verifier consumes ONLY labelled projection.
 verified=history.verify_managed_history(out/'wires',args.root)
 ((args.out if args.capture else HERE)/'transition-proof.json').write_text(json.dumps({'status':'SOURCE_ONLY_FIXED_SEMANTIC_TRANSITION_AND_UNCHANGED_HISTORY_PASS','authentication':authentication,'actual_capture_unmodified':True,'derived_is_not_capture':True,'new_manifest_sha256':sha(new_manifest),'old_manifest_sha256':sha(old_manifest),'rows':rows,'unchanged_history':verified},indent=2)+'\n')
 patch=[];pin_edits=[]
 for relative,names in [('tests/workspace/semantic_graph.rs',[k for k in current if k!='projection-wire.json']),('src/workspace_graph/type_proof_tests.rs',['projection-wire.json'])]:
  before=(args.root/relative).read_text()
  if not args.capture:
   recorded=json.loads((HERE/'complete-pin-source-inverse.json').read_text())
   matched=next(row for row in recorded if row['source']==relative)
   assert before in (matched['before'],matched['after'])
   before=matched['before']
  after=before
  for filename in names:
   prior_raw=(old/'wires'/filename).read_bytes();actual_raw=(actual/'wires'/filename).read_bytes()
   old_hash='sha256:'+sha(prior_raw);new_hash='sha256:'+sha(actual_raw)
   if args.capture:
    live_options={old_hash,'sha256:'+binding['actual_wire_sha256'][filename]}
    present=[h for h in live_options if after.count(h)==1]
    assert len(present)==1,(relative,filename,'unrecognized live pin baseline')
    old_hash=present[0]
   assert after.count(old_hash)==1,(relative,filename,'old full-wire pin occurrence')
   after=after.replace(old_hash,new_hash)
  if relative.startswith('tests/'):
   amounts=['1_111_428','1_114_864'] if args.capture else ['1_111_428']
   present=[amount for amount in amounts if after.count(amount)==1];assert len(present)==1
   after=after.replace(present[0],format(a['budget']['used_builder_bytes'],'_'))
   after=after.replace('// Restoring the prior 336449-byte debit and authenticated digest chain\n    // independently reproduces all nine original whole-document KATs.', '// Imported Point metadata now selects graph.v10; the manifest revision and\n    // analysis digest chain follow. The separate, explicitly derived history\n    // proof restores this semantic transition before unchanged budget checks.\n    // See docs/evidence/managed67b-semantic-history-20261016/README.md.')
  pin_edits.append({'source':relative,'before_sha256':sha(before.encode()),'after_sha256':sha(after.encode()),'before':before,'after':after})
  patch.extend(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/'+relative,tofile='b/'+relative))
 destination=args.out if args.capture else HERE
 (destination/'actual-only-pin-update.patch').write_text(''.join(patch))
 (destination/'complete-pin-source-inverse.json').write_text(json.dumps(pin_edits,indent=2)+'\n')
 print('PASS exact Point transition; all10 independent whole historical inverses; actual-only pin patch emitted')
if __name__=='__main__':main()
