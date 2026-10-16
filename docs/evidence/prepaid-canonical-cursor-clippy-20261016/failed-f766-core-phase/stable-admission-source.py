"""Wait at most45s for a stable safe window, then exec the unchanged strict guardian."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess,time
p=argparse.ArgumentParser();p.add_argument('--round',type=Path,required=True);p.add_argument('--phase',choices=['core','full_cli'],required=True);p.add_argument('--attempt',type=int,required=True);a=p.parse_args()
r=a.round.resolve();planpath=r/'parent-actual-launch-plan.json';plan=json.loads(planpath.read_text());m=r/'expected-counts.json'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(m)==plan['manifest_sha256'];command=plan['clippy_guardian']['command_'+a.phase]
assert sha(Path(plan['clippy_guardian']['path']))==plan['clippy_guardian']['sha256']
log=r/('stable-admission-'+a.phase+'-'+str(a.attempt)+'.jsonl');start=time.monotonic();safe_since=None
with log.open('x',buffering=1) as f:
 def emit(**row):f.write(json.dumps({'time_unix':time.time(),**row})+'\n')
 emit(event='stable_admission_identity',source=plan['source'],manifest_sha256=plan['manifest_sha256'],script_sha256=sha(Path(__file__)),maximum_wait_seconds=45,minimum_stable_seconds=8,minimum_free_percent=60,required_kernel_level=1,command=command)
 while time.monotonic()-start<45:
  level=int(subprocess.check_output(['/usr/sbin/sysctl','-n','kern.memorystatus_vm_pressure_level'],text=True,timeout=3))
  text=subprocess.check_output(['/usr/bin/memory_pressure','-Q'],text=True,timeout=3);match=re.search(r'System-wide memory free percentage:\s*(\d+)%',text);assert match
  free=int(match.group(1));ps=subprocess.check_output(['ps','-axo','comm='],text=True,timeout=3)
  heavy=sum(line.strip().rsplit('/',1)[-1] in {'cargo','rustc','clippy-driver','clang','cc1'} for line in ps.splitlines())
  now=time.monotonic();safe=level==1 and free>=60 and heavy==0
  safe_since=(now if safe_since is None else safe_since) if safe else None
  stable=0 if safe_since is None else now-safe_since
  emit(event='admission_observation',kernel_level=level,free_percent=free,heavy_process_count=heavy,stable_seconds=stable)
  if stable>=8:
   assert sha(m)==plan['manifest_sha256']
   emit(event='exec_existing_guardian',command=command,actual_phase_not_yet_started=True)
   f.flush();os.execvp(command[0],command)
  time.sleep(2)
 emit(event='admission_wait_timeout',executed_rust=False,reason='No stable normal-pressure window; source and original phase drivers unchanged')
raise SystemExit(125)
