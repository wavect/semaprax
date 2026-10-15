from pathlib import Path
import subprocess,json,time,os,signal,re
out=Path('/Volumes/Semaprax/provenance/selected-child-refusal-781-ef0cb-20261015/profile-expansion/review')
cmd=['/Users/kevin/.local/bin/claude','--print','--tools','','--strict-mcp-config','--mcp-config','{"mcpServers":{}}','--setting-sources','','--settings','{"disableAllHooks":true}','--no-session-persistence','--model','sonnet','--max-budget-usd','3.5','--output-format','json']
start=time.monotonic();peak=0;reason=None
with (out/'result.json').open('w') as log,(out/'stderr.log').open('w') as err:
 child=subprocess.Popen(cmd,cwd=out,stdin=subprocess.PIPE,stdout=log,stderr=err,text=True,start_new_session=True)
 try:
  child.stdin.write((out/'prompt.txt').read_text());child.stdin.close()
  while child.poll() is None:
   rows=subprocess.check_output(['/bin/ps','-axo','pgid=,rss='],text=True,timeout=5).splitlines()
   rss=sum(int(v[1])*1024 for row in rows if len(v:=row.split())==2 and int(v[0])==child.pid);peak=max(peak,rss)
   pressure=subprocess.check_output(['/usr/bin/memory_pressure','-Q'],text=True,timeout=5);m=re.search(r'System-wide memory free percentage: (\d+)%',pressure)
   if (out/'result.json').stat().st_size>2*1024**2:reason='2MiB output cap'
   if rss>768*1024**2:reason='768MiB review cap'
   if not m or int(m.group(1))<15:reason='system free floor unavailable or below15percent'
   if time.monotonic()-start>120:reason='120s review deadline'
   if reason:break
   time.sleep(2)
 finally:
  if child.poll() is None:
   os.killpg(child.pid,signal.SIGTERM)
   try:child.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
 receipt={'argv':cmd,'prompt_sha256':__import__('hashlib').sha256((out/'prompt.txt').read_bytes()).hexdigest(),'exit_code':child.wait(),'wall_seconds':time.monotonic()-start,'tools_enabled':False,'code_execution':False,'peak_process_group_rss_bytes':peak,'resource_refusal':reason}
 (out/'process-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True)
