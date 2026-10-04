#!/usr/bin/env python3
"""Collect bounded local LAW-16 process samples without inferring unavailable phases."""
import argparse,hashlib,json,os,pathlib,statistics,subprocess,time

def run(argv,env):
 t=time.monotonic_ns();p=subprocess.run(argv,capture_output=True,env=env); ms=(time.monotonic_ns()-t)/1e6
 return {'argv':argv,'exit_code':p.returncode,'wall_ms':ms,'stdout_sha256':'sha256:'+hashlib.sha256(p.stdout).hexdigest(),'stderr_sha256':'sha256:'+hashlib.sha256(p.stderr).hexdigest()}
def main():
 a=argparse.ArgumentParser();a.add_argument('--command',required=True);a.add_argument('--runs',type=int,default=30);a.add_argument('--output',type=pathlib.Path,required=True);x=a.parse_args()
 if x.runs<30 or x.output.exists(): raise SystemExit('runs must be >=30 and output new')
 argv=json.loads(x.command); rows=[run(argv,dict(os.environ,BEND_NO_TELEMETRY='1')) for _ in range(x.runs)]; vals=[r['wall_ms'] for r in rows]
 out={'schema':'semaprax.bend2-law-benchmark.local-process-measurement.v1','samples':rows,'statistics':{'count':len(vals),'p50_ms':statistics.median(vals),'p95_ms':sorted(vals)[max(0,round(.95*len(vals))-1)],'mean_ms':statistics.mean(vals),'population_stdev_ms':statistics.pstdev(vals)},'unavailable':['cold-state isolation','peak_memory','proof_synthesis_effort','provider_cost','cache_invalidation_work']};x.output.write_text(json.dumps(out,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
