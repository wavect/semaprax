import json,os,re,signal,subprocess,time
from pathlib import Path
ROUND=Path(__file__).resolve().parent

def observation():
 kernel=int(subprocess.check_output(['/usr/sbin/sysctl','-n','kern.memorystatus_vm_pressure_level'],text=True,timeout=5).strip())
 text=subprocess.check_output(['/usr/bin/memory_pressure','-Q'],text=True,timeout=5)
 found=re.search(r'System-wide memory free percentage:\s*(\d+)%',text)
 if not found:raise RuntimeError('free percent unavailable')
 return kernel,int(found[1])
with (ROUND/'outer-pressure.jsonl').open('x',buffering=1) as log:
 def emit(**row):log.write(json.dumps({'time_unix':time.time(),**row})+'\n')
 level,free=observation();emit(event='admission',kernel=level,free_percent=free,minimum_free_percent=60)
 if level!=1 or free<60:raise SystemExit(125)
 env=dict(os.environ,TMPDIR='/Volumes/Semaprax/runtime-tmp',PYTHONDONTWRITEBYTECODE='1')
 child=subprocess.Popen(['/opt/homebrew/bin/python3',str(ROUND/'controller.py')],cwd='/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang',env=env,start_new_session=True)
 (ROUND/'watch-child.json').write_text(json.dumps({'watch_pid':os.getpid(),'controller_pid':child.pid})+'\n')
 emit(event='controller_started',pid=child.pid)
 reason=None
 try:
  while child.poll() is None:
   level,free=observation();emit(event='observation',kernel=level,free_percent=free)
   if level==4 or free<30:
    reason='kernel critical or system free below30percent';break
   time.sleep(2)
 except BaseException as error:reason='outer observation/interruption: '+repr(error)
 if reason and child.poll() is None:
  emit(event='interrupt_controller',reason=reason,pid=child.pid)
  child.send_signal(signal.SIGINT)
 code=child.wait();emit(event='controller_terminal',exit_code=code,reason=reason)
 print(json.dumps({'controller_pid':child.pid,'exit_code':code,'outer_refusal':reason}),flush=True)
 raise SystemExit(code if code>=0 else 128-code)
