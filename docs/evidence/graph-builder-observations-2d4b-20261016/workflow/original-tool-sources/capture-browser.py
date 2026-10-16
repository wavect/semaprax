from pathlib import Path
import json,sys,subprocess,hashlib,os,time,argparse
P=argparse.ArgumentParser();P.add_argument('--out',type=Path,required=True);P.add_argument('--expected-build-receipt-sha256',required=True);P.add_argument('--expected-readiness-sha256',required=True);ARGS=P.parse_args();OUT=ARGS.out.resolve();FLOW=Path(__file__).resolve().parent
REPO=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
ROUND=Path('/Volumes/Semaprax/lane-prompts/final-8bb9-76-resume-qualification-20261016')
HEAD='8bb9f5b7b99e1e71a797460c46180daf716b234b'
sys.path.insert(0,str(ROUND/'drivers'))
from observed_phase import run_phase,provenance
sha=lambda p:hashlib.file_digest(Path(p).open('rb'),'sha256').hexdigest()
receipt=ROUND/'owning-build-result.json';build=json.loads(receipt.read_text());assert build['source_subject']==HEAD and build['exit_code']==0
record=json.loads((FLOW/'skeleton-record.json').read_text());ready_path=FLOW/'actual-completed-build-artifact-readiness.json'
assert len(ARGS.expected_build_receipt_sha256)==64 and sha(receipt)==ARGS.expected_build_receipt_sha256
assert len(ARGS.expected_readiness_sha256)==64 and sha(ready_path)==ARGS.expected_readiness_sha256
ready=json.loads(ready_path.read_text());assert ready['source_subject']==HEAD and ready['actual_build_receipt_sha256']==ARGS.expected_build_receipt_sha256
assert build['ordered_commands']==record['required_ordered_build_commands'] and len(build['results'])==4 and all(r['exit_code']==0 for r in build['results'])
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
binding={'source_subject':HEAD,'source_tree':build['source_tree'],'executable':str(EXE),'executable_sha256':EXESHA,'build_receipt':str(receipt),'build_receipt_sha256':sha(receipt),'build_metadata_sha256':sha(ROUND/'owning-build.jsonl'),'build_log_sha256':sha(ROUND/'owning-build.log'),'artifact_record':records[0],'guard':provenance(),'inputs':inputs,'producer_contract':'.github/workflows/ci.yml:979-1005','no_test_execution':True,'capture_is_not_owner_test_pass':True,'controller_sha256':sha(__file__)}
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
