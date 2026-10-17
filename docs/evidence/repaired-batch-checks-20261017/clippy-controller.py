import fcntl,hashlib,json,os,signal,subprocess,sys,time
from pathlib import Path
ROUND=Path(__file__).resolve().parent
ROOT=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
HEAD='6fb88632f73ea8aed713c5e22156c2d838cce7c5'
DRIVERS=Path('/Volumes/Semaprax/lane-prompts/final-focused258-039b15f6-20261016/drivers')
sys.path.insert(0,str(DRIVERS))
from observed_phase import run_phase

def termination(signum,frame):raise SystemExit(128+signum)
signal.signal(signal.SIGTERM,termination)
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def sha(path):
 with Path(path).open('rb') as source:return hashlib.file_digest(source,'sha256').hexdigest()
def bound():
 if git('rev-parse','HEAD')!=HEAD or git('status','--porcelain','--untracked-files=all'):raise RuntimeError('clean source HEAD drift')
bound();tree=git('rev-parse','HEAD^{tree}')
held=Path('/Volumes/Semaprax/lane-prompts/consolidated-owning-heavy.lock').open('a+')
fcntl.flock(held,fcntl.LOCK_EX|fcntl.LOCK_NB)
env=os.environ.copy()
env.update(TMPDIR='/Volumes/Semaprax/runtime-tmp',PYTHONDONTWRITEBYTECODE='1',CARGO_HOME='/Volumes/Semaprax/internal-cache-offload-20261016/cargo-home',RUSTUP_HOME='/Volumes/Semaprax/internal-cache-offload-20261016/rustup-home',CARGO_TARGET_DIR=str(ROOT/'target/final-cgu64-20261012'),CARGO_BUILD_JOBS='1',CARGO_NET_OFFLINE='true',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_DEV_CODEGEN_UNITS='64',CARGO_PROFILE_TEST_CODEGEN_UNITS='64',CARGO_PROFILE_DEV_OPT_LEVEL='0',CARGO_PROFILE_TEST_OPT_LEVEL='0',CARGO_PROFILE_DEV_DEBUG_ASSERTIONS='true',CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true',CARGO_PROFILE_DEV_OVERFLOW_CHECKS='true',CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true',CARGO_PROFILE_DEV_LTO='false',CARGO_PROFILE_TEST_LTO='false',CLANG='/usr/bin/clang',SEMAPRAX_ARCHIVER='/usr/bin/libtool',SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD=HEAD,SEMAPRAX_COLLECTION_WRITER_BUILD_TREE=tree,XDG_CACHE_HOME=str(ROUND/'cache'))
env['PATH']='/Users/kevin/.nvm/versions/node/v24.3.0/bin:'+env.get('PATH','/usr/bin:/bin')
command=['/Users/kevin/.cargo/bin/rustup','run','1.97.1','cargo','clippy','--locked','--offline','-p','semaprax','--lib','--bin','semaprax','--jobs','1','--message-format=json','--','-D','warnings']
inputs=ROUND/'source-inputs.jsonl'
with inputs.open('x') as out:
 for name in subprocess.check_output(['git','ls-files','-z'],cwd=ROOT).decode().split('\0'):
  if not name:continue
  path=ROOT/name
  digest=hashlib.sha256(os.fsencode(os.readlink(path))).hexdigest() if path.is_symlink() else sha(path)
  out.write(json.dumps({'path':name,'sha256':digest})+'\n')
receipt={'source_head':HEAD,'source_tree':tree,'cwd':str(ROOT),'command':command,'environment':{k:v for k,v in env.items() if k.startswith(('CARGO_','RUSTUP_','SEMAPRAX_')) or k in ['TMPDIR','PYTHONDONTWRITEBYTECODE','PATH','CLANG','XDG_CACHE_HOME']},'source_inputs_sha256':sha(inputs),'controller_sha256':sha(__file__),'phase_sha256':sha(DRIVERS/'observed_phase.py'),'pressure_helper_sha256':sha(DRIVERS/'semaprax_pressure_observation_v3.py'),'executed_test_bodies':0,'pid':os.getpid()}
(ROUND/'admission.json').write_text(json.dumps(receipt,indent=2)+'\n')
with (ROUND/'memory.jsonl').open('x',buffering=1) as samples,(ROUND/'cargo.jsonl').open('x') as stdout,(ROUND/'cargo.log').open('x') as stderr:
 def emit(row):
  samples.write(json.dumps(row)+'\n')
  if row.get('label')=='guard' and not (ROUND/'child.json').exists():
   processes=row.get('processes',[])
   (ROUND/'child.json').write_text(json.dumps({'controller':os.getpid(),'processes':processes,'source_head':HEAD,'command':command},indent=2)+'\n')
 result=run_phase(command,cwd=ROOT,env=env,stdout=stdout,stderr=stderr,rss_limit_bytes=9*1024**3,emit=emit,bind=bound)
receipt.update(result=result,cargo_log_sha256=sha(ROUND/'cargo.log'),cargo_jsonl_sha256=sha(ROUND/'cargo.jsonl'),memory_sha256=sha(ROUND/'memory.jsonl'))
(ROUND/'result.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'source_head':HEAD,'result':result['exit_code'],'resource_refusal':result['resource_refusal'],'group_settlement':result['group_settlement']}),flush=True)
raise SystemExit(result['exit_code'] if result['exit_code']>=0 else 128-result['exit_code'])
