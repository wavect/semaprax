from pathlib import Path
from observed_phase import run_phase, provenance, observation
from draft_contract import verify_frozen, private_target, prepare_environment
def sha_file(path):
 with Path(path).open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

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
import json,subprocess,os,time,re,signal,hashlib,tomllib,shutil

# Termination must unwind the watchdog finally block and settle its own child.
def settle_on_termination(signum, frame):
 raise SystemExit(128+signum)
signal.signal(signal.SIGTERM,settle_on_termination)

OUT=Path(os.environ['SEMAPRAX_CHECK_ROUND'])
m=json.loads((OUT/'expected-counts.json').read_text());subject=m.get('source_subject');ROOT=Path(m['draft_repository']).resolve()
verify_frozen(m, ROOT)
runtime_manifest_sha256=hashlib.sha256((OUT/'expected-counts.json').read_bytes()).hexdigest()
def bound():
 verify_frozen(m, ROOT)
 if hashlib.sha256((OUT/'expected-counts.json').read_bytes()).hexdigest()!=runtime_manifest_sha256:raise SystemExit('Runtime manifest drift')
 if subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()!=subject or subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT,text=True).strip():raise SystemExit('Source drift')
bound()
import fcntl
heavy_lock=(OUT.parent/'consolidated-owning-heavy.lock').open('a+')
fcntl.flock(heavy_lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
host_rows=subprocess.check_output(['ps','-axo','pgid=,comm='],text=True,timeout=5)
if len({line.split()[0] for line in host_rows.splitlines() if line.split() and line.split()[-1].rsplit('/',1)[-1] in {'cargo','rustc','clippy-driver','clang','cc1'}})>=2:raise SystemExit('Two unrelated heavy host groups already active')
env=os.environ.copy();node_version=prepare_environment(env, OUT);env['TMPDIR']=str((OUT/'runtime-tmp').resolve());env.update(SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD=subject,SEMAPRAX_COLLECTION_WRITER_BUILD_TREE=m['source_tree'],CARGO_TARGET_DIR=str(private_target(ROOT)),CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CLANG='/usr/bin/clang',SEMAPRAX_ARCHIVER='/usr/bin/libtool')
env.update(CARGO_PROFILE_DEV_CODEGEN_UNITS='64',CARGO_PROFILE_TEST_CODEGEN_UNITS='64',CARGO_PROFILE_DEV_OPT_LEVEL='0',CARGO_PROFILE_TEST_OPT_LEVEL='0',CARGO_PROFILE_DEV_DEBUG_ASSERTIONS='true',CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true',CARGO_PROFILE_DEV_OVERFLOW_CHECKS='true',CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true',CARGO_PROFILE_DEV_LTO='false',CARGO_PROFILE_TEST_LTO='false')
artifacts={}
for line in (OUT/'owning-build.jsonl').read_text().splitlines():
 try:r=json.loads(line)
 except ValueError:continue
 if r.get('reason')=='compiler-artifact' and r.get('profile',{}).get('test') and r.get('executable'):
  kinds=r['target']['kind'];target='lib' if kinds==['lib'] else r['target']['name'];package=tomllib.loads(Path(r['manifest_path']).read_text())['package']['name'];key=package+'::'+target;artifacts[key]=r['executable']
build_receipt_path=OUT/'owning-build-result.json'
build_receipt=json.loads(build_receipt_path.read_text())
if build_receipt.get('source_subject')!=subject or build_receipt.get('exit_code')!=0:raise SystemExit('Unqualified compiler artifacts')
assert build_receipt['manifest_sha256']==runtime_manifest_sha256
assert build_receipt['source_tree']==m['source_tree']
assert build_receipt['owning_build_metadata_sha256']==sha_file(OUT/'owning-build.jsonl')
build_receipt_sha=sha_file(build_receipt_path)
build_metadata_sha=sha_file(OUT/'owning-build.jsonl')
built_hashes={str(Path(a['executable']).resolve()):a['executable_sha256'] for a in build_receipt['artifacts']}
artifact_hashes={}
for key,value in artifacts.items():
 path=Path(value).resolve()
 if not path.is_relative_to(private_target(ROOT)) or built_hashes.get(str(path))!=sha_file(path):raise SystemExit('Build-receipt executable binding drift '+key)
 artifact_hashes[key]=built_hashes[str(path)]
env.pop('SEMAPRAX_NATIVE_TEST_DIAGNOSTICS', None)
driver_sha256=sha_file(__file__)
results=[]
priority={'codegen::selected_final_utf8::tests::':0,'cleanup_plan::phase_measurement::paired_routes_preserve_complete_program_plans_and_independent_replay':1,'cleanup_plan::build::helper_index::attached_capacity::flat::tests::':2,'cleanup_plan::build::helper_index::tests::':3}
acceptance_priority={selector: index for index, issue in enumerate(m.get('issue_acceptance', [])) for selector in issue['selectors']}
for i,row in sorted(enumerate(m['checks']),key=lambda pair:(acceptance_priority.get(pair[1].get('selector'), len(m.get('issue_acceptance', []))), priority.get(pair[1].get('selector'),4),pair[0])):
 cmd=row['command']
 if i < 0 or not row.get('selector'):continue
 bound();key=cmd[cmd.index('--test')+1] if '--test' in cmd else (cmd[cmd.index('--bin')+1] if '--bin' in cmd else 'lib');key=cmd[cmd.index('-p')+1]+'::'+key;exe=artifacts[key]
 if sha_file(exe)!=artifact_hashes[key]:raise SystemExit('Executable drift before selected test')
 tail=cmd[cmd.index('--')+1:];args=[exe,row['selector'],*tail]
 listed=subprocess.run([exe,row['selector'],'--list','--format','terse']+(['--exact'] if '--exact' in tail else []),cwd=ROOT,env=env,text=True,capture_output=True)
 names=[l[:-6] for l in listed.stdout.splitlines() if l.endswith(': test')];skips=[tail[j+1] for j,v in enumerate(tail[:-1]) if v=='--skip'];names=[n for n in names if not any(s in n for s in skips)]
 if listed.returncode or names != [row['selector']]:raise SystemExit('Failed, zero, extra, or wrong fully qualified discovery '+row['selector'])
 expected_names=sorted(b['name'] for b in row['selected_bodies']+row.get('ignored_bodies_not_executed',[]))
 if sorted(n.rsplit('::',1)[-1] for n in names)!=expected_names:raise SystemExit('Actual discovery differs from authored source census '+row['selector'])
 log=OUT/f'owning-selected-{i:02d}.log';start=time.monotonic();timeout=False
 run_env=env.copy();diagnostic_capture=None
 if row['selector']=='native_test::recovery::timeout_settles_before_next_case_and_authenticated_reentry':
  diagnostic_capture=str((OUT/'native-timeout-diagnostics').resolve())
  run_env['SEMAPRAX_NATIVE_TEST_DIAGNOSTICS']=diagnostic_capture
 samples_path=OUT/f'owning-selected-{i:02d}-memory-samples.jsonl'
 if log.exists() or samples_path.exists():raise SystemExit('Retained selected evidence exists; choose fresh round')
 samples_path.touch()
 def owned_bound():
  bound()
  if sha_file(exe)!=artifact_hashes[key]:raise SystemExit('Executable drift after selected test')
 with log.open('w') as out:
  def emit_sample(sample):
   with samples_path.open('a') as samples:samples.write(json.dumps(sample)+'\n')
  monitored=run_phase(args,cwd=ROOT,env=run_env,stdout=out,stderr=subprocess.STDOUT,rss_limit_bytes=8*1024**3,emit=emit_sample,wall_limit=600,wall_deadline_inclusive=True,disk_free=lambda:shutil.disk_usage(ROOT).free,bind=owned_bound,command_index=i)
 code=monitored['exit_code'];timeout=monitored['resource_refusal']=='phase time cap';resource_refusal=monitored['resource_refusal'];peak_rss=monitored['peak_process_group_rss_bytes']
 summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',log.read_text());summary=summaries[-1] if summaries else None
 try:after_hash=sha_file(exe)
 except OSError as artifact_error:
  after_hash=None;code=1;resource_refusal=observation.first_refusal(resource_refusal,'Executable drift after selected test');monitored['observation_errors'].append(str(artifact_error))
 result=dict(**{k:v for k,v in monitored.items() if k not in {'command','exit_code','resource_refusal','peak_process_group_rss_bytes','wall_seconds'}},memory_samples_path=str(samples_path),memory_samples_sha256=hashlib.sha256(samples_path.read_bytes()).hexdigest(),runtime_manifest_sha256=runtime_manifest_sha256,driver_sha256=driver_sha256,node_version=node_version,private_target_dir=str(private_target(ROOT)),diagnostic_capture=diagnostic_capture,qualification_wall_comparison_eligible=diagnostic_capture is None,executable_sha256=artifact_hashes[key],executable_sha256_before=artifact_hashes[key],executable_sha256_after=after_hash,owning_build_receipt_sha256=build_receipt_sha,owning_build_metadata_sha256=build_metadata_sha,source_subject=subject,harness=key,selector=row['selector'],command=args,executable=exe,discovered_names=names,expected_selected_static=row['expected_selected'],exit_code=code,timed_out=timeout,resource_refusal=resource_refusal,peak_process_group_rss_bytes=peak_rss,wall_seconds=time.monotonic()-start,log=str(log),log_sha256=sha_file(log),summary=summary)
 results.append(result);(OUT/'owning-selected-results.json').write_text(json.dumps(dict(source_subject=subject,complete=False,results=results),indent=2)+'\n');print(len(results),key,row['selector'],code,summary,flush=True)
 if resource_refusal:break
final_binding_refusal=None
try:bound()
except BaseException as binding_error:final_binding_refusal='Source/manifest drift: '+str(binding_error)
complete=final_binding_refusal is None and len(results)==sum(i >= 0 and bool(row.get('selector')) for i,row in enumerate(m['checks']))
(OUT/'owning-selected-results.json').write_text(json.dumps(dict(source_subject=subject,complete=complete,final_binding_refusal=final_binding_refusal,**provenance(),results=results),indent=2)+'\n')
print(json.dumps({'source':subject,'groups':len(results),'failed_groups':sum(r['exit_code']!=0 for r in results),'static_count_mismatches':[r['selector'] for r in results if r['summary'] and int(r['summary'][1])+int(r['summary'][2])!=r['expected_selected_static']]}),flush=True)

raise SystemExit(not complete or any(r['exit_code'] or not r['summary'] or int(r['summary'][1])!=r['expected_selected_static'] or int(r['summary'][2]) or int(r['summary'][3]) for r in results))
