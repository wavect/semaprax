import sys,os,json,subprocess,hashlib,fcntl
from pathlib import Path
sys.path.insert(0,'/Volumes/Semaprax/lane-prompts/final-d4cab-142-qualification-20261015/drivers')
from observed_phase import run_phase
from draft_contract import prepare_environment,private_target
root=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');head='85142274c2f3e73bc1a4e5150672150c4d17d95f'
out=Path('/Volumes/Semaprax/provenance/diagnostic-851-caller-20261015');out.mkdir(exist_ok=False)
lock=(Path('/Volumes/Semaprax/lane-prompts')/'consolidated-owning-heavy.lock').open('a+');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
def bound():
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==head
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
bound();env=os.environ.copy();prepare_environment(env,out)
env.update(SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD=head,SEMAPRAX_COLLECTION_WRITER_BUILD_TREE=subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=root,text=True).strip(),CARGO_TARGET_DIR=str(private_target(root)),CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',CLANG='/usr/bin/clang',SEMAPRAX_ARCHIVER='/usr/bin/libtool',TMPDIR=str(out/'runtime-tmp'))
for profile in ['DEV','TEST']:
 for k,v in dict(CODEGEN_UNITS='64',OPT_LEVEL='0',DEBUG_ASSERTIONS='true',OVERFLOW_CHECKS='true',LTO='false').items():env['CARGO_PROFILE_'+profile+'_'+k]=v
commands=[('production-build',['/Users/kevin/.cargo/bin/rustup','run','1.97.1','cargo','build','--locked','--offline','-p','semaprax-toolchain','--bin','semaprax-full','--jobs','1','--message-format=json'],7*1024**3,900),('normalizer-diagnostic',[str(private_target(root)/'debug/semaprax-full'),'check',str(root/'examples/catalog-normalizer-project'),'--json'],2*1024**3,180)]
for name,cmd,cap,wall in commands:
 samples=out/(name+'-memory.jsonl');log=out/(name+'.log')
 def emit(x):
  with samples.open('a')as f:f.write(json.dumps(x)+'\n')
 with log.open('w')as f:r=run_phase(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT,rss_limit_bytes=cap,emit=emit,wall_limit=wall,bind=bound)
 r.update(source_subject=head,command=cmd,diagnostic_discovery_only=True,log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),no_test_bodies=True)
 if name=='normalizer-diagnostic':r['executable_sha256']=hashlib.sha256(Path(cmd[0]).read_bytes()).hexdigest()
 (out/(name+'-result.json')).write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:r[k]for k in ['source_subject','exit_code','wall_seconds','peak_process_group_rss_bytes','resource_refusal']}),flush=True)
 if name=='production-build'and r['exit_code']:raise SystemExit(r['exit_code'])
