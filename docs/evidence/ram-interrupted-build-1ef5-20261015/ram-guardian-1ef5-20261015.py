import json,os,signal,subprocess,time
from pathlib import Path
pid=60957
expected="python3 /Volumes/Semaprax/lane-prompts/large-1ef5-qualification-20261015/drivers/build-owning.py --round /Volumes/Semaprax/lane-prompts/large-1ef5-qualification-20261015 --rss-limit-gib 9"
log=Path("/Volumes/Semaprax/provenance/ram-guardian-1ef5-20261015.jsonl")
warning_since=None
with log.open("x",buffering=1) as f:
 while True:
  p=subprocess.run(["/bin/ps","-p",str(pid),"-o","command="],capture_output=True,text=True,timeout=3)
  if p.returncode or not p.stdout.strip():
   f.write(json.dumps({"event":"controller_terminal","time":time.time()})+"\n");break
  if p.stdout.strip()!=expected:
   f.write(json.dumps({"event":"identity_changed_no_signal","time":time.time()})+"\n");break
  level=int(subprocess.check_output(["/usr/sbin/sysctl","-n","kern.memorystatus_vm_pressure_level"],text=True,timeout=3).strip())
  now=time.monotonic()
  if level==2:
   if warning_since is None: warning_since=now
  else: warning_since=None
  duration=0 if warning_since is None else now-warning_since
  row={"time":time.time(),"kernel_pressure":level,"warning_seconds":duration,"controller_pid":pid}
  if level==4 or duration>=20:
   row["event"]="interrupt_owned_controller";f.write(json.dumps(row)+"\n")
   os.kill(pid,signal.SIGINT);break
  row["event"]="observation";f.write(json.dumps(row)+"\n");time.sleep(2)
