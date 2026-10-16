from pathlib import Path
import json,sys,subprocess,hashlib,os,time,argparse
P=argparse.ArgumentParser();P.add_argument('--out',type=Path,required=True);P.add_argument('--expected-build-receipt-sha256',required=True);P.add_argument('--expected-readiness-sha256',required=True);ARGS=P.parse_args();OUT=ARGS.out.resolve();FLOW=Path(__file__).resolve().parent
REPO=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
ROUND=Path('/Volumes/Semaprax/lane-prompts/final-47f-72-qualification-20261016')
HEAD='47f80b0f7a06fde9b2388439a9c0bfe998a05af1'
sys.path.insert(0,str(ROUND/'drivers'))
from observed_phase import run_phase,provenance
sha=lambda p:hashlib.file_digest(Path(p).open('rb'),'sha256').hexdigest()
receipt=ROUND/'owning-build-result.json';build=json.loads(receipt.read_text());assert build['source_subject']==HEAD and build['exit_code']==0
record=json.loads((FLOW/'skeleton-record.json').read_text());ready_path=FLOW/'actual-completed-build-artifact-readiness.json'
assert len(ARGS.expected_build_receipt_sha256)==64 and sha(receipt)==ARGS.expected_build_receipt_sha256
assert len(ARGS.expected_readiness_sha256)==64 and sha(ready_path)==ARGS.expected_readiness_sha256
ready=json.loads(ready_path.read_text());assert ready['source_subject']==HEAD and ready['actual_build_receipt_sha256']==ARGS.expected_build_receipt_sha256
assert build['ordered_commands']==record['required_ordered_build_commands'] and len(build['results'])==5 and all(r['exit_code']==0 for r in build['results'])
assert not OUT.exists() and not OUT.is_relative_to(REPO)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip()==HEAD and not subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=REPO).strip()
OUT.mkdir()

assert sha(ROUND/'owning-build.jsonl')==build['owning_build_metadata_sha256'];assert sha(ROUND/'owning-build.log')==build['log_sha256']
records=[]
for line in (ROUND/'owning-build.jsonl').open():
 try:r=json.loads(line)
 except:continue
 if r.get('reason')=='compiler-artifact' and r.get('target',{}).get('name')=='semaprax' and r.get('executable') and not r.get('profile',{}).get('test'):records.append(r)
assert len(records)==1
EXE=Path(records[0]['executable']); EXESHA=sha(EXE)
assert str(EXE.resolve())==ready['production_CLI_artifacts_from_same_completed_build']['semaprax']['executable'] and EXESHA==ready['production_CLI_artifacts_from_same_completed_build']['semaprax']['executable_sha256']
inputs=[];fixtures=[]
for name in ['baseline','renamed']:
 root=OUT/name;root.mkdir();(root/'src').mkdir();fixtures.append(root)
 for p in ['semaprax.toml','src/app.spx','src/core.spx','src/tests.spx']:
  source_path='examples/calculator-project/'+p
  raw=subprocess.check_output(['git','show',HEAD+':'+source_path],cwd=REPO)
  original=raw
  if name=='renamed' and p=='src/core.spx':
   assert raw.count(b'\nfn add(')==1;raw=raw.replace(b'\nfn add(',b'\nfn sum(',1)
  (root/p).write_bytes(raw)
  inputs.append({'fixture':name,'path':p,'source_path':source_path,'git_original_sha256':hashlib.sha256(original).hexdigest(),'actual_sha256':sha(root/p),'bytes':len(raw),'rename_only':raw!=original})
binding={'source_subject':HEAD,'source_tree':build['source_tree'],'executable':str(EXE),'executable_sha256':EXESHA,'build_receipt':str(receipt),'build_receipt_sha256':sha(receipt),'build_metadata_sha256':sha(ROUND/'owning-build.jsonl'),'build_log_sha256':sha(ROUND/'owning-build.log'),'artifact_record':records[0],'guard':provenance(),'inputs':inputs,'producer_contract':'.github/workflows/ci.yml exact frozen2d4b / Main eb13 browser graph capture','producer_contract_sha256':sha(REPO/'.github/workflows/ci.yml'),'no_test_execution':True,'capture_is_not_owner_test_pass':True,'controller_sha256':sha(__file__)}
(OUT/'binding.json').write_text(json.dumps(binding,indent=2)+'\n')
def bound():
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip()==HEAD and not subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=REPO).strip()
 assert sha(ready_path)==ARGS.expected_readiness_sha256
 assert sha(EXE)==EXESHA and sha(receipt)==binding['build_receipt_sha256']
 for row in inputs:assert sha(OUT/row['fixture']/row['path'])==row['actual_sha256']
env=os.environ.copy();temp=OUT/'runtime-tmp';temp.mkdir();env['TMPDIR']=str(temp);env['TMPPREFIX']=str(temp/'zsh')
results=[]
def domain_digest(wire):
 d=json.loads(wire); field=(',"graph_digest":'+json.dumps(d['graph_digest'],separators=(',',':'))).encode();assert wire.count(field)==1
 payload=wire.replace(field,b'',1);return 'sha256:'+hashlib.sha256(b'semaprax.project-semantic-graph.artifact-digest.v1\0'+len(payload).to_bytes(8,'little')+payload).hexdigest()
for i,root in enumerate(fixtures):
 bound();name=root.name;wire=OUT/f'calculator-{name}.json';err=OUT/f'{name}.stderr.log';samples=OUT/f'{name}.memory.jsonl'
 def emit(row):
  with samples.open('a') as f:f.write(json.dumps(row)+'\n')
 with wire.open('xb') as stdout,err.open('xb') as stderr:
  result=run_phase([str(EXE),'graph',str(root/'semaprax.toml')],cwd=root,env=env,stdout=stdout,stderr=stderr,rss_limit_bytes=7*1024**3,emit=emit,wall_limit=120,wall_deadline_inclusive=True,bind=bound,command_index=i)
 bound();result.update(fixture=name,source_subject=HEAD,executable_sha256_after=sha(EXE),wire_sha256=sha(wire),stderr_sha256=sha(err))
 results.append(result);(OUT/'results.json').write_text(json.dumps({'source_subject':HEAD,'complete':False,'results':results},indent=2)+'\n')
 print(name,result['exit_code'],result.get('resource_refusal'),flush=True)
 if result['exit_code']!=0 or result.get('resource_refusal'):raise SystemExit('producer refused; evidence preserved')
 raw=wire.read_bytes();assert raw.endswith(b'\n');raw=raw[:-1];d=json.loads(raw);assert domain_digest(raw)==d['graph_digest']
 mainpath=REPO/f'docs/evidence/browser-graph-pins-2600e49eb/current-hosted-calculator-{name}.json';assert sha(mainpath)==record['browser_required_prior']['Main2600-'+name]['sha256'];mainwire=mainpath.read_bytes().removesuffix(b'\n');maindata=json.loads(mainwire)
 assert domain_digest(mainwire)==maindata['graph_digest'] and maindata['budget']['used_builder_bytes']==3296148
 actual_bill=d['budget']['used_builder_bytes'];actual_field=f'"used_builder_bytes":{actual_bill}'.encode();assert raw.count(actual_field)==1
 mainrestore=raw.replace(actual_field,b'"used_builder_bytes":3296148',1)
 mainfield=('"graph_digest":'+json.dumps(d['graph_digest'])).encode();assert mainrestore.count(mainfield)==1
 mainrestore=mainrestore.replace(mainfield,('"graph_digest":'+json.dumps(maindata['graph_digest'])).encode(),1)
 assert mainrestore==mainwire,'OTHER FIELD OR BYTE DIFFERENCE FROM FULL MAIN2600: stop without pin edits'
 observedpath=Path('/Volumes/Semaprax/provenance/graph-builder-observations-8bb9-20261016/browser')/f'calculator-{name}.json';assert sha(observedpath)==record['browser_required_prior']['observed8bb-'+name]['sha256'];observed=observedpath.read_bytes().removesuffix(b'\n');obs=json.loads(observed)
 assert domain_digest(observed)==obs['graph_digest'] and obs['budget']['used_builder_bytes']==3534564
 observedrestore=raw.replace(actual_field,b'"used_builder_bytes":3534564',1).replace(mainfield,('"graph_digest":'+json.dumps(obs['graph_digest'])).encode(),1)
 assert observedrestore==observed,'OTHER FIELD OR BYTE DIFFERENCE FROM ACTUAL8bb: stop without pin edits'
 result['new_independent_bridges']={'main2600_wire_sha256':sha(mainpath),'main2600_builder_bytes':3296148,'observed8bb_wire_sha256':sha(observedpath),'observed8bb_builder_bytes':3534564,'complete_main2600_restore':True,'complete_observed8bb_restore':True}
 observed2d_path=FLOW/'historical-observations/observed2d4b-browser'/f'calculator-{name}.json'
 assert sha(observed2d_path)==record['browser_required_prior']['observed2d4b-'+name]['sha256']
 observed2d_wire=observed2d_path.read_bytes().removesuffix(b'\n');observed2d=json.loads(observed2d_wire)
 assert domain_digest(observed2d_wire)==observed2d['graph_digest'] and observed2d['budget']['used_builder_bytes']==3558886
 observed2d_restore=raw.replace(actual_field,b'"used_builder_bytes":3558886',1).replace(mainfield,('"graph_digest":'+json.dumps(observed2d['graph_digest'])).encode(),1)
 assert observed2d_restore==observed2d_wire,'OTHER FIELD OR BYTE DIFFERENCE FROM FULL ACTUAL2d4b: stop without pin edits'
 result['new_independent_bridges'].update(observed2d4b_wire_sha256=sha(observed2d_path),observed2d4b_builder_bytes=3558886,complete_observed2d4b_restore=True)
 observed364d_path=FLOW/'historical-observations/observed364d-browser'/f'calculator-{name}.json'
 assert sha(observed364d_path)==record['browser_required_prior']['observed364d-'+name]['sha256']
 observed364d_wire=observed364d_path.read_bytes().removesuffix(b'\n');observed364d=json.loads(observed364d_wire)
 assert domain_digest(observed364d_wire)==observed364d['graph_digest'] and observed364d['budget']['used_builder_bytes']==3472870
 observed364d_restore=raw.replace(actual_field,b'"used_builder_bytes":3472870',1).replace(mainfield,('"graph_digest":'+json.dumps(observed364d['graph_digest'])).encode(),1)
 assert observed364d_restore==observed364d_wire,'OTHER FIELD OR BYTE DIFFERENCE FROM FULL ACTUAL364d: stop without pin edits'
 result['new_independent_bridges'].update(observed364d_wire_sha256=sha(observed364d_path),observed364d_builder_bytes=3472870,complete_observed364d_restore=True)
 priorpath=REPO/f'docs/evidence/browser-graph-pins-35cc-20261016/actual-capture/calculator-{name}.json';prior=priorpath.read_bytes().removesuffix(b'\n');p=json.loads(prior);assert domain_digest(prior)==p['graph_digest']
 assert p['budget']['used_builder_bytes']==4032239
 bill=d['budget']['used_builder_bytes'];field=f'"used_builder_bytes":{bill}'.encode();assert raw.count(field)==1
 restored=raw.replace(field,b'"used_builder_bytes":4032239',1)
 digfield=('"graph_digest":'+json.dumps(d['graph_digest'])).encode();assert restored.count(digfield)==1
 restored=restored.replace(digfield,('"graph_digest":'+json.dumps(p['graph_digest'])).encode(),1)
 assert restored==prior,'OTHER FIELD OR BYTE DIFFERENCE: stop without pin edits'
 middlepath=REPO/f'docs/evidence/browser-graph-pins-17104e20c/current-hosted-calculator-{name}.json';middle=middlepath.read_bytes().removesuffix(b'\n');mid=json.loads(middle);assert domain_digest(middle)==mid['graph_digest'] and mid['budget']['used_builder_bytes']==3930276
 bridge=prior.replace(b'"used_builder_bytes":4032239',b'"used_builder_bytes":3930276',1).replace(('"graph_digest":'+json.dumps(p['graph_digest'])).encode(),('"graph_digest":'+json.dumps(mid['graph_digest'])).encode(),1);assert bridge==middle
 historicalpath=REPO/f'docs/evidence/browser-graph-pins-841efe3e2/historical-calculator-{name}.json';historical=historicalpath.read_bytes().removesuffix(b'\n');h=json.loads(historical);assert h['budget']['used_builder_bytes']==1214409;assert domain_digest(historical)==h['graph_digest']
 old=prior.replace(b'"used_builder_bytes":4032239',b'"used_builder_bytes":1214409',1).replace(('"graph_digest":'+json.dumps(p['graph_digest'])).encode(),('"graph_digest":'+json.dumps(h['graph_digest'])).encode(),1);assert old==historical
 result['validation']={'actual_builder_bytes':bill,'prior_builder_bytes':4032239,'historical_builder_bytes':1214409,'actual_graph_digest':d['graph_digest'],'prior_wire_sha256':sha(priorpath),'historical_wire_sha256':sha(historicalpath),'complete_prior_after_only_bill_and_digest_restore':True,'complete_original_after_only_bill_and_digest_restore':True}
 (OUT/'results.json').write_text(json.dumps({'source_subject':HEAD,'complete':i==1,'results':results},indent=2)+'\n')
