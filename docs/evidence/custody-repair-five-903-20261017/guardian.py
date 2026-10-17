"""Frozen5 build/runtime wrapper: single-job55-admit/30-stop policy; original full body retained.
Prepared utility only. Invocation after registration/freeze starts actual phases.
"""
from pathlib import Path
from collections import deque
import argparse,hashlib,importlib.util,json,os,re,shutil,signal,subprocess,time

p=argparse.ArgumentParser(description=__doc__)
p.add_argument("--round",type=Path,required=True)
p.add_argument("--phase",choices=["build","runtime"],required=True)
a=p.parse_args()
r=a.round.resolve()
SSD=Path("/Volumes/Semaprax")
ROOT_EXPECTED=Path("/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang")
EXPECTED_DRIVERS={'run-owning-selected.py': 'fb7459b6e44a336e44f779de6aff9fbf7921d986f86a8157aedb2c810eed6450', 'draft_contract.py': '1869cf5aa5633a55f05323589a3db3d092fd6e98b6f558f025f6faebab3fa038', 'semaprax_pressure_observation_v3.py': '5f1d756491ccc5dd662afe6e7ed744b7e65e8e0a45bc7e420b358ec531cec90b', 'run-production-lib-cli-clippy.py': 'bc67ef6dbed2bc2423b25159bc995c4a66dba489b5b4e326635351196e5241d9', 'build-owning.py': '07cf91ca745f3e53f3f6dee51fca407734dc75c7d2316efb5f8e96d079c731a5', 'observed_phase.py': '36d75093b66bdb07e059a9255e9b7decebf4a2a2c10a531543b202b24b302867'}
ORIGINAL_WATCHES={'build-guardian.py': {'original': '/Volumes/Semaprax/lane-prompts/final-focused258-039b15f6-20261016/pressure-guardian-source.py', 'sha256': '8188fc7040a3ce3557971243059f9621cad74c76d5cc71b02d7d9bec43534de4', 'bytes': 6397}, 'runtime-guardian.py': {'original': '/Volumes/Semaprax/lane-prompts/final-focused258-039b15f6-20261016/selected-pressure-guardian-source.py', 'sha256': '4e7c4816d521ca3520676ee63884c0f384869fb547e55599fe83f5c18b81a1a2', 'bytes': 6889}, 'stable-admission.py': {'original': '/Volumes/Semaprax/provenance/wait-stable-build-admission60-67b1b-20261016.py', 'sha256': '5bb28175b8ac6d9f42d8e67643ccf31dcb3a7647a305f8f70d23ad4d8e8f0eb9', 'bytes': 2443}}
def digest(path):
    with path.open("rb") as stream:return hashlib.file_digest(stream,"sha256").hexdigest()
def require(condition,message):
    if not condition:raise SystemExit(message)
require(r.is_relative_to(SSD) and r.is_dir(),"Fresh prepared SSD round required")
require(not r.is_relative_to(ROOT_EXPECTED),"Round must be outside shipping checkout")
m=json.loads((r/"expected-counts.json").read_text())
manifest_sha=digest(r/"expected-counts.json")
require(m.get("source_subject") and not m.get("freeze_required"),"Frozen source required")
root=Path(m["draft_repository"]).resolve()
require(root==ROOT_EXPECTED,"Private shipping ROOT target required")
require(not subprocess.check_output(["git","status","--porcelain=v1","--untracked-files=all"],cwd=root,text=True).strip(),"ROOT must be clean/frozen")
require(subprocess.check_output(["git","rev-parse","HEAD"],cwd=root,text=True).strip()==m["source_subject"],"Frozen HEAD drift")
require(subprocess.check_output(["git","rev-parse","HEAD^{tree}"],cwd=root,text=True).strip()==m["source_tree"],"Frozen tree drift")
for relative,expected in m["source_file_sha256"].items():
    path=(root/relative).resolve()
    require(path.is_relative_to(root) and digest(path)==expected,"Source binding drift: "+relative)
checks=m["checks"]
require(len(checks)==5 and len({(row["package"],row["harness"]) for row in checks})==1,"Expected exact5 checks/1 literal roles")
require(len({(row["package"],row["harness"],row["selector"]) for row in checks})==5,"Duplicate owning selector")
require(not m.get("compile_only_harnesses"),"No extra compile-only harness authorized")
for row in checks:
    require(row["expected_selected"]==1 and row["command"].count("--exact")==1,"Exact single-owner checks required")
    require(row["command"][-1]=="--test-threads=1","Serial owning execution required")
registration=r/"registration.json";selection=r/"selection.json"
require(digest(registration)==m["registration_sha256"],"Registration drift")
require(digest(selection)==m["selection_sha256"],"Selection drift")
reg=json.loads(registration.read_text())
require(reg["source_head"]==m["source_subject"] and reg["source_tree"]==m["source_tree"],"Registration not current frozen source")
require(reg["repository"]==str(root) and len(reg["rows"])==5 and not reg["issues"],"Incomplete/foreign registration")
require(reg["selection_sha256"]==m["selection_sha256"],"Registration selection mismatch")
receipt=json.loads((r/"preparation-receipt.json").read_text())
require(receipt["source"]==m["source_subject"] and receipt["manifest_sha256"]==manifest_sha,"Preparation binding drift")
for name,expected in EXPECTED_DRIVERS.items():
    require(digest(r/"drivers"/name)==expected,"Original prepared driver drift: "+name)
    require(m["prepared_driver_sha256"][name]==expected and receipt["driver_sha256"][name]==expected,"Driver receipt drift: "+name)
require((r/"runtime-tmp").is_dir() and (r/"runtime-tmp").resolve().is_relative_to(SSD),"Prepared SSD tmp required")
os.environ["TMPDIR"]=str((r/"runtime-tmp").resolve())
os.environ["PYTHONDONTWRITEBYTECODE"]="1"
os.environ["SEMAPRAX_CHECK_ROUND"]=str(r)
a.rss_limit_gib=8
if a.phase=="build":
    require(not (r/"owning-build-result.json").exists(),"Retained build result exists; choose fresh round")
    driver=r/"drivers/build-owning.py"
    command=["/opt/homebrew/bin/python3",str(driver),"--round",str(r),"--rss-limit-gib","8"]
else:
    build=json.loads((r/"owning-build-result.json").read_text())
    require(build["exit_code"]==0 and build["source_subject"]==m["source_subject"] and build["source_tree"]==m["source_tree"],"Successful same-source build required")
    require(build["manifest_sha256"]==manifest_sha,"Build manifest drift")
    require(not (r/"owning-selected-results.json").exists(),"Retained runtime result exists; choose fresh round")
    driver=r/"drivers/run-owning-selected.py"
    command=["/opt/homebrew/bin/python3",str(driver)]
require(not (r/(a.phase+"-outer-pressure-guardian.jsonl")).exists(),"Outer watch log already exists")
require(not (r/(a.phase+"-outer-pressure-guardian-source.py")).exists(),"Outer watch source already exists")
binding={"schema":"semaprax.outer-watch-binding.v1","source":m["source_subject"],"manifest_sha256":manifest_sha,"phase":a.phase,"original_watches":ORIGINAL_WATCHES,"original_driver_sha256":EXPECTED_DRIVERS,"command":command,"production_equivalence_claimed":False,"policy":"user-authorized single-job55-admit30-stop; runtime inner8GiB unchanged; build8GiB; original heavylock/jobs1/private target","execution_performed_by_preparation":False}
with (r/(a.phase+"-outer-watch-binding.json")).open("x") as stream:json.dump(binding,stream,indent=2);stream.write("\n")
helper=r/"drivers/semaprax_pressure_observation_v3.py"
spec=importlib.util.spec_from_file_location("bound_pressure_observation",helper);observation=importlib.util.module_from_spec(spec);spec.loader.exec_module(observation)
def swap_used():return observation.decode_swapusage(subprocess.check_output(["/usr/sbin/sysctl","-b","vm.swapusage"],timeout=3))["used_bytes"]
def free_percent():
 raw=subprocess.check_output(["/usr/bin/memory_pressure","-Q"],text=True,timeout=3)
 match=re.search(r"System-wide memory free percentage:\s*(\d+)%",raw)
 if not match:raise RuntimeError("memory free percentage unavailable")
 return int(match.group(1))
log=r/(a.phase+"-outer-pressure-guardian.jsonl");warning_since=None;warning_swap=None;interrupted=False
low_since=None;very_low_since=None;history=deque()
with log.open("x",buffering=1) as f:
 def emit(**row):f.write(json.dumps({"time_unix":time.time(),**row})+"\n")
 guardian_source=Path(__file__).resolve();raw=guardian_source.read_bytes();shutil.copyfile(guardian_source,r/(a.phase+"-outer-pressure-guardian-source.py"))
 emit(event="guardian_identity",source=str(guardian_source),source_sha256=hashlib.sha256(raw).hexdigest(),rss_limit_gib=a.rss_limit_gib,build_driver=str(driver),build_driver_sha256=hashlib.sha256(driver.read_bytes()).hexdigest())
 initial=subprocess.run(["/usr/sbin/sysctl","-n","kern.memorystatus_vm_pressure_level"],capture_output=True,text=True,timeout=3)
 admission_free=free_percent()
 admission_disk_free=shutil.disk_usage("/private/var/vm").free
 if initial.returncode or initial.stdout.strip() not in ("1",) or admission_free<55 or admission_disk_free<5*1024**3:
  emit(event="admission_refused",kernel_raw=initial.stdout.strip(),free_percent=admission_free,returncode=initial.returncode,internal_disk_free_bytes=admission_disk_free,minimum_internal_disk_free_bytes=5*1024**3);raise SystemExit(125)
 emit(event="admission_accepted",kernel_raw=initial.stdout.strip(),free_percent=admission_free,minimum_admission_free_percent=55,internal_disk_free_bytes=admission_disk_free,minimum_internal_disk_free_bytes=5*1024**3)
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
