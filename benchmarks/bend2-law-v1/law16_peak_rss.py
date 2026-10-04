#!/usr/bin/env python3
import argparse,json,os,re,subprocess,time,pathlib
p=argparse.ArgumentParser();p.add_argument('--command',required=True);p.add_argument('--runs',type=int,default=30);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
if a.runs<30 or a.output.exists():raise SystemExit('need new output and >=30 runs')
cmd=json.loads(a.command); rows=[]
for _ in range(a.runs):
 t=time.monotonic_ns();x=subprocess.run(['/usr/bin/time','-l',*cmd],capture_output=True,env=dict(os.environ,BEND_NO_TELEMETRY='1'));m=re.search(rb'(\d+)\s+maximum resident set size',x.stderr);rows.append({'command':cmd,'wrapper':['/usr/bin/time','-l'],'exit_code':x.returncode,'wall_ms':(time.monotonic_ns()-t)/1e6,'peak_rss_bytes':int(m.group(1)) if m else None,'stderr':x.stderr.decode('utf-8','replace')})
a.output.write_text(json.dumps({'schema':'semaprax.bend2-law-benchmark.peak-rss.v1','samples':rows,'nonclaims':['RSS includes /usr/bin/time wrapper observation']},indent=2,sort_keys=True)+'\n')
