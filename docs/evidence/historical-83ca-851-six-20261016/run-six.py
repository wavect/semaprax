#!/usr/bin/env python3
"""Explicit historical83ca artifact supplement; never current0c/97 qualification."""
import argparse,hashlib,json,os,pathlib,re,shutil,signal,subprocess,sys,time,fcntl
HERE=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--expected-plan-sha256',required=True);p.add_argument('--heavy-free-confirmed',action='store_true');p.add_argument('--out',required=True,type=pathlib.Path);a=p.parse_args()
def sha(path):
 with pathlib.Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
assert a.heavy_free_confirmed,'Parent HEAVYFREE confirmation required'
assert sha(HERE/'plan.json')==a.expected_plan_sha256
m=json.loads((HERE/'plan.json').read_text());ROOT=pathlib.Path(m['repository']);R=pathlib.Path(m['producer_round']);EXE=m['executable']
for f,s in m['drivers'].items():assert sha(R/'drivers'/f)==s
sys.path.insert(0,str(R/'drivers'))
from observed_phase import run_phase,provenance
from draft_contract import prepare_environment
# Exact original phase monitor and observation helper imported, never rewritten.
old_receipt=json.loads((R/'owning-build-result.json').read_text());assert old_receipt['exit_code']==0 and old_receipt['source_subject']==m['artifact_source_head']
assert len(old_receipt['results'])==3 and all(x['exit_code']==0 and not x.get('resource_refusal') and x['group_settlement']['confirmed_empty'] for x in old_receipt['results'])
def bound():
 assert sha(HERE/'plan.json')==a.expected_plan_sha256
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==m['cwd_source_head']
 assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT,text=True).strip()
 assert sha(EXE)==m['executable_sha256']
 for path,key in [(R/'owning-build-result.json','build_receipt_sha256'),(R/'owning-build.jsonl','build_metadata_sha256'),(R/'expected-counts.json','producer_manifest_sha256'),(pathlib.Path(m['production_equivalence_receipt']),'production_equivalence_receipt_sha256')]:assert sha(path)==m[key]
 for row in m['unchanged_dependency_files']:assert sha(ROOT/row['path'])==row['sha256']
 for f,s in m['drivers'].items():assert sha(R/'drivers'/f)==s
bound()
# Same shared heavy lock and stricter parent-authorized zero unrelated compile groups.
lock=(R.parent/'consolidated-owning-heavy.lock').open('a+');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
rows=subprocess.check_output(['ps','-axo','pgid=,comm='],text=True,timeout=5)
assert not any(x.split() and x.split()[-1].rsplit('/',1)[-1] in {'cargo','rustc','clippy-driver','clang','cc1'} for x in rows.splitlines()),'Host heavy job active'
assert not a.out.exists(),'Fresh output directory required';a.out.mkdir(parents=True)
env=os.environ.copy();node=prepare_environment(env,a.out);env['TMPDIR']='/Volumes/Semaprax/runtime-tmp';env.pop('SEMAPRAX_NATIVE_TEST_DIAGNOSTICS',None)
env.update(SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD=m['artifact_source_head'],SEMAPRAX_COLLECTION_WRITER_BUILD_TREE=old_receipt['source_tree'])
signal.signal(signal.SIGTERM,lambda s,f:(_ for _ in ()).throw(SystemExit(128+s)))
results=[]
def publish(complete):
 payload={'schema':'semaprax.historical-exact-artifact-supplement.v1','status':'ACTUAL_HISTORICAL_83CA_SUPPLEMENT','artifact_source_head':m['artifact_source_head'],'cwd_source_head':m['cwd_source_head'],'current0c97_PASS':False,'original97_status':m['current97_status'],'plan_sha256':a.expected_plan_sha256,'driver_sha256':sha(__file__),'complete':complete,'node_version':node,'results':results,**provenance()}
 (a.out/'results.json').write_text(json.dumps(payload,indent=2)+'\n')
for i,row in enumerate(m['checks']):
 bound();selector=row['selector'];listed=subprocess.run([EXE,selector,'--exact','--list','--format','terse'],cwd=ROOT,env=env,text=True,capture_output=True,timeout=30)
 (a.out/f'discovery-{i:02d}.json').write_text(json.dumps({'command':listed.args,'exit_code':listed.returncode,'stdout':listed.stdout,'stderr':listed.stderr},indent=2)+'\n')
 names=[x[:-6] for x in listed.stdout.splitlines() if x.endswith(': test')];assert listed.returncode==0 and names==[selector]
 log=a.out/f'owner-{i:02d}.log';samples=a.out/f'owner-{i:02d}-memory.jsonl';samples.touch()
 def emit(x):
  with samples.open('a') as f:f.write(json.dumps(x)+'\n')
 with log.open('w') as output:
  observed=run_phase([EXE,selector,'--exact','--nocapture','--test-threads=1'],cwd=ROOT,env=env,stdout=output,stderr=subprocess.STDOUT,rss_limit_bytes=8*1024**3,emit=emit,wall_limit=600,wall_deadline_inclusive=True,disk_free=lambda:shutil.disk_usage(ROOT).free,bind=bound,command_index=i)
 summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',log.read_text());summary=summaries[-1] if summaries else None
 valid=observed['exit_code']==0 and not observed['resource_refusal'] and not observed['observation_errors'] and observed['group_settlement']['confirmed_empty'] and summary==('ok','1','0','0')
 results.append({**observed,'selector':selector,'discovered_names':names,'expected_selected_static':1,'summary':summary,'valid_PASS':valid,'log':str(log),'log_sha256':sha(log),'memory_samples':str(samples),'memory_samples_sha256':sha(samples),'executable_sha256':sha(EXE),'artifact_source_head':m['artifact_source_head'],'current0c97_PASS':False});publish(False)
 print(i,selector,observed['exit_code'],summary,flush=True)
 if observed['resource_refusal'] or observed['observation_errors']:break
bound();publish(len(results)==6)
raise SystemExit(not(len(results)==6 and all(x['valid_PASS'] for x in results)))
