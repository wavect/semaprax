from pathlib import Path
import argparse,json,sys,subprocess,hashlib,os,fcntl,tomllib
ap=argparse.ArgumentParser();ap.add_argument('--round',required=True);ap.add_argument('--out',required=True);a=ap.parse_args();round=Path(a.round).resolve();out=Path(a.out).resolve();assert not out.exists();sys.path.insert(0,str(round/'drivers'))
from observed_phase import run_phase
from draft_contract import verify_frozen,private_target,prepare_environment
m=json.loads((round/'expected-counts.json').read_text());root=Path(m['draft_repository']);verify_frozen(m,root)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
b=json.loads((round/'owning-build-result.json').read_text());assert b['exit_code']==0 and b['source_subject']==m['source_subject'];built={str(Path(v['executable']).resolve()):v['executable_sha256'] for v in b['artifacts']};found={}
for line in (round/'owning-build.jsonl').read_text().splitlines():
 try:v=json.loads(line)
 except ValueError:continue
 if v.get('reason')!='compiler-artifact' or not v.get('profile',{}).get('test') or not v.get('executable'):continue
 if tomllib.loads(Path(v['manifest_path']).read_text())['package']['name']!='semaprax':continue
 name=v['target']['name']
 if (name=='semaprax' and v['target']['kind']==['lib']) or (name=='workspace' and v['target']['kind']==['test']):
  assert name not in found, 'ambiguous owning binary role'
  exe=Path(v['executable']).resolve();assert exe.is_relative_to(private_target(root)) and built[str(exe)]==sha(exe);found[name]=exe
assert set(found)=={'semaprax','workspace'}
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
env=os.environ.copy();prepare_environment(env,round);env['TMPDIR']=str(round/'runtime-tmp');env['CARGO_TARGET_DIR']=str(private_target(root))
lock=(round.parent/'consolidated-owning-heavy.lock').open('a+');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
def bound():
 verify_frozen(m,root)
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
 for exe in found.values():assert built[str(exe)]==sha(exe)
log=round/'prelude-pin-producers.log';samples=round/'prelude-pin-producers-memory.jsonl';receipt=round/'prelude-pin-producers-result.json';assert not any(x.exists() for x in [log,samples,receipt]);samples.touch()
def emit(v):
 with samples.open('a') as f:f.write(json.dumps(v)+'\n')
cmd=['/bin/sh',str(root/'scripts/capture-prelude-forecast-pins.sh'),str(found['semaprax']),str(found['workspace']),str(out)]
with log.open('w') as f:r=run_phase(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT,rss_limit_bytes=2*1024**3,emit=emit,wall_limit=180,wall_deadline_inclusive=True,bind=bound)
bound();r.update(source_subject=m['source_subject'],command=cmd,capture_out=str(out),build_receipt_sha256=sha(round/'owning-build-result.json'),log_sha256=sha(log),capture_success_is_not_owner_pass=True,capture_driver_sha256=sha(__file__),binary_roles={n:{'executable':str(p),'sha256':sha(p)} for n,p in found.items()});receipt.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:r[k] for k in ['exit_code','resource_refusal','peak_process_group_rss_bytes','wall_seconds','capture_out']}));raise SystemExit(r['exit_code'])
