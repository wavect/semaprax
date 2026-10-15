#!/usr/bin/env python3
"""PREPARED ONLY. Execute only after root schedules the frozen consolidated build."""
from pathlib import Path
import argparse,fcntl,hashlib,json,os,shutil,signal,subprocess,time,re
from observed_phase import run_phase, provenance
from draft_contract import verify_frozen, private_target, prepare_environment

# Termination must unwind the watchdog finally block and settle its own child.
def settle_on_termination(signum, frame):
 raise SystemExit(128+signum)
signal.signal(signal.SIGTERM,settle_on_termination)

P=argparse.ArgumentParser();P.add_argument('--round',required=True);P.add_argument('--rss-limit-gib',type=float,default=9);a=P.parse_args();OUT=Path(a.round).resolve();m=json.loads((OUT/'expected-counts.json').read_text());ROOT=Path(m['draft_repository']).resolve();subject=m.get('source_subject')
verify_frozen(m, ROOT)
def sha(p):
 with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
manifest_sha=sha(OUT/'expected-counts.json')
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def bound():
 verify_frozen(m, ROOT)
 if git('rev-parse','HEAD')!=subject or git('status','--porcelain=v1','--untracked-files=all') or sha(OUT/'expected-counts.json')!=manifest_sha:raise SystemExit('Source/manifest drift')
bound();tree=git('rev-parse','HEAD^{tree}');lock_sha=sha(ROOT/'Cargo.lock')
if a.rss_limit_gib>9:raise SystemExit('Shared-machine policy caps heavy build process group at 9GiB')
pressure=subprocess.check_output(['/usr/bin/memory_pressure','-Q'],text=True,timeout=5)
match=re.search(r'System-wide memory free percentage: (\d+)%',pressure)
if not match or int(match.group(1))<30:raise SystemExit('Heavy-job admission requires observed system memory free >=30percent')
if (OUT/'owning-build-result.json').exists():raise SystemExit('Retained build receipt already exists; choose fresh round')
held=(OUT.parent/'consolidated-owning-heavy.lock').open('a+')
try:fcntl.flock(held,fcntl.LOCK_EX|fcntl.LOCK_NB)
except BlockingIOError:raise SystemExit('Another coordinated heavy phase holds the lock')
ps=subprocess.check_output(['ps','-axo','pgid=,comm='],text=True,timeout=5)
if len({line.split()[0] for line in ps.splitlines() if line.split() and line.split()[-1].rsplit('/',1)[-1] in {'cargo','rustc','clippy-driver','clang','cc1'}})>=2:raise SystemExit('Two unrelated heavy host groups already active')
if shutil.disk_usage(ROOT).free<5*1024**3:raise SystemExit('5GiB disk floor')
TARGET=private_target(ROOT);env=os.environ.copy();node_version=prepare_environment(env, OUT);env['TMPDIR']=str((OUT/'runtime-tmp').resolve());env.update(SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD=subject,SEMAPRAX_COLLECTION_WRITER_BUILD_TREE=tree,CARGO_TARGET_DIR=str(TARGET),CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',CLANG='/usr/bin/clang',SEMAPRAX_ARCHIVER='/usr/bin/libtool')
env.update(CARGO_PROFILE_DEV_CODEGEN_UNITS='64',CARGO_PROFILE_TEST_CODEGEN_UNITS='64',CARGO_PROFILE_DEV_OPT_LEVEL='0',CARGO_PROFILE_TEST_OPT_LEVEL='0',CARGO_PROFILE_DEV_DEBUG_ASSERTIONS='true',CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true',CARGO_PROFILE_DEV_OVERFLOW_CHECKS='true',CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true',CARGO_PROFILE_DEV_LTO='false',CARGO_PROFILE_TEST_LTO='false')
def planned_commands(manifest, toolchain):
 # A target must be named literally: no wildcard expands the authorized owner set.
 def literal(value):
  if not isinstance(value,str) or not re.fullmatch(r'[A-Za-z0-9_-]+',value):
   raise ValueError('Owning targets require explicit literal names')
  return value
 main=set();extras=set()
 for row in manifest['checks']:
  command=row['command'];package=literal(command[command.index('-p')+1])
  if '--test' in command:target=('--test',literal(command[command.index('--test')+1]))
  elif '--bin' in command:target=('--bin',literal(command[command.index('--bin')+1]))
  elif '--lib' in command:target=('--lib','')
  else:raise ValueError('Selected row has no explicit owning target')
  if package=='semaprax':main.add(target)
  else:extras.add((package,*target))
 for target in manifest.get('compile_only_harnesses',[]):main.add(('--test',literal(target)))
 commands=[]
 if ('--lib','') in main:
  commands.append(toolchain+['cargo','test','--locked','--offline','-p','semaprax','--lib','--no-run','--jobs','1','--message-format=json'])
  main.remove(('--lib',''))
 commands.append(toolchain+['cargo','build','--locked','--offline','-p','semaprax','--lib','--jobs','1','--message-format=json'])
 if main:
  selected=[]
  for kind,name in sorted(main,key=lambda x:(x[0]!='--lib',x[0],x[1])):
   selected += [kind]+([name] if name else [])
  commands.append(toolchain+['cargo','test','--locked','--offline','-p','semaprax']+selected+['--no-run','--jobs','1','--message-format=json'])
 for package,kind,name in sorted(extras):
  commands.append(toolchain+['cargo','test','--locked','--offline','-p',package,kind]+([name] if name else [])+['--no-run','--jobs','1','--message-format=json'])
 core_unit_command = toolchain+['cargo','test','--locked','--offline','-p','semaprax','--lib','--no-run','--jobs','1','--message-format=json']
 if core_unit_command in commands:
  commands.append(commands.pop(commands.index(core_unit_command)))
 return commands

rustup=['/Users/kevin/.cargo/bin/rustup','run','1.97.1']
commands=planned_commands(m,rustup)
# Only the exact selected core lib unit no-run command moves to last.
# Production lib and all other original literal batches retain relative order.
# External package owners remain separately and explicitly selected.
# CARGO_BIN_EXE production binaries required by integration harnesses remain
# ordinary prerequisites; no binary unit, example or doc harness is added.
samples_path=OUT/'memory-samples.jsonl'
if samples_path.exists():raise SystemExit('Retained memory samples already exist; choose fresh round')
samples_path.touch()
results=[];start=time.monotonic()
progress_path=OUT/'owning-build-progress.json'
if progress_path.exists():raise SystemExit('Retained progress receipt already exists; choose fresh round')
def write_progress():
 progress=dict(**provenance(),complete=False,source_subject=subject,source_tree=tree,cargo_lock_sha256=lock_sha,manifest_sha256=manifest_sha,driver_sha256=sha(__file__),ordered_commands=commands,cargo_build_jobs=1,build_target_selection='explicit-batched-core-lib-unit-last-no-run-v1',results=results,memory_samples_sha256=sha(samples_path),executed_test_bodies=0,node_version=node_version,private_target_dir=str(TARGET))
 progress_path.write_text(json.dumps(progress,indent=2)+'\n')
for name in ['owning-build.jsonl','owning-build.log']:
 if (OUT/name).exists():raise SystemExit('Retained build log already exists; choose fresh round')
 (OUT/name).touch()
for i,cmd in enumerate(commands):
 refusal=None;admission_free=None
 try:
  admission_started=time.monotonic()
  while True:
   bound()
   admission=subprocess.check_output(['/usr/bin/memory_pressure','-Q'],text=True,timeout=5)
   match=re.search(r'System-wide memory free percentage: (\d+)%',admission)
   admission_free=int(match.group(1)) if match else None
   if admission_free is None or admission_free>=30 or time.monotonic()-admission_started>=90:break
   time.sleep(5)
  if admission_free is None or admission_free<30:refusal='Serial child admission requires system memory free >=30percent after bounded90s wait'
  if shutil.disk_usage(ROOT).free<5*1024**3:refusal='5GiB disk floor before serial child'
 except BaseException as observation_error:
  refusal='Serial admission observation failed: '+type(observation_error).__name__
 if refusal:
  results.append(dict(command=cmd,executed=False,exit_code=125,resource_refusal=refusal,peak_process_group_rss_bytes=0,last_system_free_percent=admission_free,process_high_water_marks=[],wall_seconds=0,**provenance()))
  write_progress()
  break
 with (OUT/"owning-build.jsonl").open("a") as stdout,(OUT/"owning-build.log").open("a") as stderr:
  def emit_sample(row):
   with samples_path.open("a") as samples:samples.write(json.dumps(row)+"\n")
  result=run_phase(cmd,cwd=ROOT,env=env,stdout=stdout,stderr=stderr,rss_limit_bytes=int(a.rss_limit_gib*1024**3),emit=emit_sample,command_index=i,disk_free=lambda:shutil.disk_usage(ROOT).free,bind=bound)
 results.append(result)
 write_progress()
 if result["exit_code"]:break
artifacts=[];artifact_refusals=[]
for line in (OUT/'owning-build.jsonl').read_text().splitlines():
 try:row=json.loads(line)
 except ValueError:continue
 if row.get('reason')=='compiler-artifact' and row.get('executable') and row.get('profile',{}).get('test'):
  p=Path(row['executable']).resolve()
  if not p.is_relative_to(TARGET.resolve()):
   artifact_refusals.append('Compiler artifact outside private target');break
  try:artifacts.append(dict(package_id=row['package_id'],target=row['target'],executable=str(p),executable_sha256=sha(p)))
  except (OSError,KeyError) as artifact_error:
   artifact_refusals.append(type(artifact_error).__name__+': '+str(artifact_error));break
receipt=dict(**provenance(),schema='semaprax.owning-build-bound.v3',source_subject=subject,source_tree=tree,cargo_lock_sha256=lock_sha,manifest_sha256=manifest_sha,driver_sha256=sha(__file__),ordered_commands=commands,cargo_build_jobs=1,build_target_selection='explicit-batched-core-lib-unit-last-no-run-v1',memory_samples_sha256=sha(OUT/'memory-samples.jsonl'),results=results,exit_code=next((r['exit_code'] for r in results if r['exit_code']),1 if artifact_refusals else 0),wall_seconds=time.monotonic()-start,executed_test_bodies=0,artifact_refusals=artifact_refusals,artifacts=artifacts,owning_build_metadata_sha256=sha(OUT/'owning-build.jsonl'),log_sha256=sha(OUT/'owning-build.log'),node_version=node_version,private_target_dir=str(TARGET))
(OUT/'owning-build-result.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:receipt[k] for k in ['source_subject','exit_code','wall_seconds','executed_test_bodies']}));raise SystemExit(bool(receipt['exit_code']))
