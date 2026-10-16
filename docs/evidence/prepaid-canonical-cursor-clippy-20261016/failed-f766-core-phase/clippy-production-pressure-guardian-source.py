"""Run one unchanged focused build driver; interrupt only its owned controller on RAM pressure."""
import argparse,hashlib,importlib.util,json,os,re,shutil,signal,subprocess,time
from pathlib import Path
from collections import deque
p=argparse.ArgumentParser();p.add_argument("--round",type=Path,required=True);p.add_argument("--rss-limit-gib",type=float,default=7);p.add_argument("--phase",choices=["clippy-production","clippy-production-toolchain-full-cli"],required=True);a=p.parse_args()
r=a.round.resolve();driver=r/"drivers"/("run-production-lib-cli-clippy.py" if a.phase=="clippy-production" else "run-production-toolchain-full-cli.py")
assert driver.is_file() and 0<a.rss_limit_gib<=9
assert not (r/(a.phase+"-result.json")).exists()
command=["python3",str(driver),"--round",str(r),"--phase",a.phase]
helper=r/"drivers/semaprax_pressure_observation_v3.py"
spec=importlib.util.spec_from_file_location("bound_pressure_observation",helper);observation=importlib.util.module_from_spec(spec);spec.loader.exec_module(observation)
def swap_used():return observation.decode_swapusage(subprocess.check_output(["/usr/sbin/sysctl","-b","vm.swapusage"],timeout=3))["used_bytes"]
def free_percent():
 raw=subprocess.check_output(["/usr/bin/memory_pressure","-Q"],text=True,timeout=3)
 match=re.search(r"System-wide memory free percentage:\s*(\d+)%",raw)
 if not match:raise RuntimeError("memory free percentage unavailable")
 return int(match.group(1))
log=r/(a.phase+"-pressure-guardian.jsonl");warning_since=None;warning_swap=None;interrupted=False
low_since=None;very_low_since=None;history=deque()
with log.open("x",buffering=1) as f:
 def emit(**row):f.write(json.dumps({"time_unix":time.time(),**row})+"\n")
 guardian_source=Path(__file__).resolve();raw=guardian_source.read_bytes();shutil.copyfile(guardian_source,r/(a.phase+"-pressure-guardian-source.py"))
 emit(event="guardian_identity",source=str(guardian_source),source_sha256=hashlib.sha256(raw).hexdigest(),rss_limit_gib=a.rss_limit_gib,build_driver=str(driver),build_driver_sha256=hashlib.sha256(driver.read_bytes()).hexdigest())
 initial=subprocess.run(["/usr/sbin/sysctl","-n","kern.memorystatus_vm_pressure_level"],capture_output=True,text=True,timeout=3)
 admission_free=free_percent()
 admission_disk_free=shutil.disk_usage("/private/var/vm").free
 if initial.returncode or initial.stdout.strip() not in ("1",) or admission_free<60 or admission_disk_free<5*1024**3:
  emit(event="admission_refused",kernel_raw=initial.stdout.strip(),free_percent=admission_free,returncode=initial.returncode,internal_disk_free_bytes=admission_disk_free,minimum_internal_disk_free_bytes=5*1024**3);raise SystemExit(125)
 emit(event="admission_accepted",kernel_raw=initial.stdout.strip(),free_percent=admission_free,minimum_admission_free_percent=60,internal_disk_free_bytes=admission_disk_free,minimum_internal_disk_free_bytes=5*1024**3)
 child=subprocess.Popen(command,start_new_session=True)
 emit(event="controller_started",pid=child.pid,command=command,rolling_growth_window_seconds=20,rolling_swap_growth_limit_bytes=256*1024**2,low_headroom_percent=30,low_headroom_sustain_seconds=30,very_low_headroom_percent=25,very_low_sustain_seconds=20,minimum_free_percent=30,internal_disk_floor_bytes=3*1024**3,critical_kernel_level=4,observation_helper_sha256=hashlib.sha256(helper.read_bytes()).hexdigest())
 try:
  while child.poll() is None:
   reason=None;now=time.monotonic()
   try:
    sample=subprocess.run(["/usr/sbin/sysctl","-n","kern.memorystatus_vm_pressure_level"],capture_output=True,text=True,timeout=3)
    if sample.returncode:raise RuntimeError("sysctl returned "+str(sample.returncode))
    level=int(sample.stdout.strip())
    if level not in (1,2,4):raise RuntimeError("unknown kernel pressure "+str(level))
    used=swap_used();free=free_percent()
    if level==2:
     if warning_since is None:warning_since=now;warning_swap=used
    else:warning_since=None;warning_swap=None
    duration=0 if warning_since is None else now-warning_since
    growth=0 if warning_swap is None else max(0,used-warning_swap)
    history.append((now,used))
    while len(history)>1 and history[1][0]<=now-20:history.popleft()
    coverage=now-history[0][0]
    rolling_growth=max(0,used-history[0][1]) if coverage>=20 else 0
    if level==2 and free<30:
     if low_since is None:low_since=now
    else:low_since=None
    if level==2 and free<25:
     if very_low_since is None:very_low_since=now
    else:very_low_since=None
    low_duration=0 if low_since is None else now-low_since
    very_low_duration=0 if very_low_since is None else now-very_low_since
    internal_disk_free=shutil.disk_usage("/private/var/vm").free
    if level==4:reason="kernel critical pressure"
    elif free<30:reason="system free below30percent"
    elif internal_disk_free<3*1024**3:reason="internal swap disk below3GiB"
    elif very_low_duration>=20:reason="sustained warning below25percent for20seconds"
    elif low_duration>=30 and rolling_growth>=256*1024**2:reason="sustained warning below30percent with rolling20second swap growth"
    emit(event="observation",kernel_level=level,warning_seconds=duration,free_percent=free,swap_used_bytes=used,warning_swap_growth_bytes=growth,rolling_window_seconds=coverage,rolling_swap_growth_bytes=rolling_growth,low_headroom_seconds=low_duration,very_low_headroom_seconds=very_low_duration,internal_disk_free_bytes=internal_disk_free)
   except Exception as error:reason="RAM observation failed: "+type(error).__name__;emit(event="observation_error",error=str(error))
   if reason:
    emit(event="interrupt_owned_controller",pid=child.pid,signal=signal.SIGINT,reason=reason)
    try:os.kill(child.pid,signal.SIGINT);interrupted=True
    except ProcessLookupError:emit(event="controller_already_terminal_no_signal")
    break
   time.sleep(2)
 except BaseException as error:
  emit(event="guardian_interrupted",error_type=type(error).__name__)
  if child.poll() is None and not interrupted:
   emit(event="interrupt_owned_controller",pid=child.pid,signal=signal.SIGINT,reason="guardian interrupted; settle owned child before leaving")
   try:os.kill(child.pid,signal.SIGINT);interrupted=True
   except ProcessLookupError:pass
  child.wait()
  raise
 code=child.wait();emit(event="controller_terminal",pid=child.pid,exit_code=code,interrupted=interrupted)
 print(json.dumps({"controller_exit_code":code,"pressure_interrupted":interrupted,"guardian_log":str(log)}))
 raise SystemExit(code if code>=0 else 128-code)
