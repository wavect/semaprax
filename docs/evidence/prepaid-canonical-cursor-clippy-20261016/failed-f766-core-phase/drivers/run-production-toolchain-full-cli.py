"""Execute one source-bound focused heavy phase with bounded RAM; no full gates."""
import argparse,fcntl,hashlib,json,os,signal,subprocess,time,shutil
from observed_phase import run_phase, provenance
from draft_contract import verify_frozen, private_target, prepare_environment

# Termination must unwind the watchdog finally block and settle its own child.
def settle_on_termination(signum, frame):
 raise SystemExit(128+signum)
signal.signal(signal.SIGTERM,settle_on_termination)


def observed_system_free_percent():
 import re
 observed=subprocess.check_output(['/usr/bin/memory_pressure','-Q'],text=True,timeout=5)
 match=re.search(r'System-wide memory free percentage: (\d+)%',observed)
 if not match:raise RuntimeError('System memory observation unavailable')
 return int(match.group(1))
def admit_heavy_phase():
 if observed_system_free_percent()<30:raise SystemExit('Heavy-job admission requires system memory free >=30percent')
def memory_stop_reason():
 try:
  if observed_system_free_percent()<15:return 'system memory free below15percent'
 except (RuntimeError,subprocess.SubprocessError):return 'system memory observation unavailable'
 return None
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--round',required=True);p.add_argument('--phase',choices=['clippy-production-toolchain-full-cli'],required=True);a=p.parse_args()
out=Path(a.round).resolve();m=json.loads((out/'expected-counts.json').read_text());root=Path(m['draft_repository']);subject=m['source_subject'];manifest_sha=hashlib.sha256((out/'expected-counts.json').read_bytes()).hexdigest()
def bound():
 verify_frozen(m, root)
 if subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()!=subject or subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=root,text=True).strip():raise SystemExit('Source drift')
 if hashlib.sha256((out/'expected-counts.json').read_bytes()).hexdigest()!=manifest_sha:raise SystemExit('Manifest drift')
bound();lock=(out.parent/'consolidated-owning-heavy.lock').open('a+')
fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
ps=subprocess.check_output(['ps','-axo','pgid=,comm='],text=True,timeout=5)
if len({line.split()[0] for line in ps.splitlines() if line.split() and line.split()[-1].rsplit('/',1)[-1] in {'cargo','rustc','clippy-driver','clang','cc1'}})>=2:raise SystemExit('Two unrelated heavy host groups already active')
env=os.environ.copy();node_version=prepare_environment(env, out);env['TMPDIR']=str((out/'runtime-tmp').resolve());env.update(SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD=subject,SEMAPRAX_COLLECTION_WRITER_BUILD_TREE=m['source_tree'],CARGO_TARGET_DIR=str(private_target(root)),CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CLANG='/usr/bin/clang',SEMAPRAX_ARCHIVER='/usr/bin/libtool')
env.update(CARGO_PROFILE_DEV_CODEGEN_UNITS='64',CARGO_PROFILE_TEST_CODEGEN_UNITS='64',CARGO_PROFILE_DEV_OPT_LEVEL='0',CARGO_PROFILE_TEST_OPT_LEVEL='0',CARGO_PROFILE_DEV_DEBUG_ASSERTIONS='true',CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true',CARGO_PROFILE_DEV_OVERFLOW_CHECKS='true',CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true',CARGO_PROFILE_DEV_LTO='false',CARGO_PROFILE_TEST_LTO='false')
selector='implementation::tests::ledger_capacity::hir_capacity_layout_constants_are_bound_to_root_const_assertions'
if a.phase=='interop-owner':
 selector='implementation::tests::row_loan_closed::row_loan_arguments_and_descriptor_are_censused_and_disposed_but_scalar_abi_refuses'
if a.phase=='interop-copy-discard-census':
 selector='implementation::tests::row_loan_closed::copy_discard_value_is_censused_and_disposed_but_scalar_abi_refuses'
if a.phase=='interop-copy-discard-c-refusal':
 selector='implementation::artifacts::c_replay::copy_discard_tests::independent_replay_refuses_discard_before_output_or_counters'
if a.phase=='unit-typecheck':
 args=['check','--locked','--offline','-p','semaprax','--lib','--profile','test']
elif a.phase=='clippy-production-toolchain-full-cli':
 args=['clippy','--locked','--offline','-p','semaprax-toolchain','--bin','semaprax-full','--','-D','warnings']
elif a.phase=='clippy':
 args=['clippy','--locked','--offline','-p','semaprax','--lib','--bin','semaprax','--','-D','warnings']
elif a.phase=='interop-clippy':args=['clippy','--locked','--offline','-p','semaprax-native-rust-interop','--lib','--','-D','warnings']
else:args=['test','--locked','--offline','-p','semaprax-native-rust-interop','--lib',selector,'--','--exact','--nocapture','--test-threads=1']
cmd=['/Users/kevin/.cargo/bin/rustup','run','1.97.1','cargo',*args]
log=out/(a.phase+'.log');receipt=out/(a.phase+'-result.json');samples_path=out/(a.phase+'-memory-samples.jsonl')
assert not receipt.exists() and not log.exists() and not samples_path.exists()
samples_path.touch();start=time.monotonic()
# run_phase reobserves the existing >=30percent admission after baseline.
with log.open('w') as f:
 def emit_sample(row):
  with samples_path.open('a') as samples:samples.write(json.dumps(row)+'\n')
 monitored=run_phase(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT,rss_limit_bytes=8*1024**3,emit=emit_sample,wall_limit=900,disk_free=lambda:shutil.disk_usage(root).free,bind=bound)
code=monitored['exit_code']
if a.phase in {'unit-typecheck','clippy-production-toolchain-full-cli','clippy','interop-clippy'}:monitored['executed_test_bodies']=0
r=dict(node_version=node_version,private_target_dir=str(private_target(root)),source_subject=subject,source_tree=m['source_tree'],manifest_sha256=manifest_sha,**monitored,log=str(log),log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),driver_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),memory_samples_path=str(samples_path),memory_samples_sha256=hashlib.sha256(samples_path.read_bytes()).hexdigest())
if a.phase in {'layout-pin','interop-owner','interop-copy-discard-census','interop-copy-discard-c-refusal'}:
 import re
 summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',log.read_text());r['summary']=summaries[-1] if summaries else None
 if not r['summary'] or r['summary'][1:]!=('1','0','0'):r['count_refusal']=True;code=1;r['exit_code']=1
receipt.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r));raise SystemExit(bool(code))
