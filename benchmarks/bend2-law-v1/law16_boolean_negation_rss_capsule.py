#!/usr/bin/env python3
"""Verify matched Boolean-negation peak-RSS sample files without comparing routes."""
import argparse,hashlib,json,pathlib,statistics
ROOT=pathlib.Path(__file__).parent;SCHEMA='semaprax.bend2-law-benchmark.boolean-negation-peak-rss-review.v1'
def digest(p):return 'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
def route(root,name,input_path):
 v=json.loads((root/name).read_text()); rows=v.get('samples')
 if v.get('schema')!='semaprax.bend2-law-benchmark.peak-rss.v1' or not isinstance(rows,list) or len(rows)!=30:raise ValueError(f'{name} lacks 30 samples')
 rss=[]
 for row in rows:
  if row.get('exit_code')!=0 or row.get('wrapper')!=['/usr/bin/time','-l'] or not isinstance(row.get('peak_rss_bytes'),int) or row['peak_rss_bytes']<=0:raise ValueError(f'{name} has invalid sample')
  rss.append(row['peak_rss_bytes'])
 if any(str(input_path) not in row.get('command',[]) for row in rows):raise ValueError(f'{name} command does not bind retained input')
 ordered=sorted(rss);return {'samples':30,'p50_peak_rss_bytes':statistics.median(rss),'p95_peak_rss_bytes':ordered[28],'input_sha256':digest(input_path),'wrapper':'/usr/bin/time -l'}
def review(root):
 root=root.resolve();bend=root/'bend-input.bend';sem=root/'semaprax-project/src/app.spx'
 if bend.read_bytes()!=(ROOT/'fixtures/bend-boolean-negation-v1.bend').read_bytes() or sem.read_bytes()!=(ROOT/'fixtures/semaprax-boolean-negation-v1.spx').read_bytes():raise ValueError('retained RSS source differs from matched fixture')
 return {'schema':SCHEMA,'status':'local_matched_rss_authenticated','routes':{'bend_verdict':route(root,'bend-verdict.json',bend),'semaprax_z3':route(root,'semaprax-z3.json',root/'semaprax-project/semaprax.toml')},'nonclaims':['RSS includes /usr/bin/time wrapper observation','no RSS ratio or winner','distinct trusted computing bases','not cold-cache isolation']}
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--capsule',required=True,type=pathlib.Path);p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args(argv)
 if a.output.exists() or not a.output.parent.is_dir():p.error('output must be new')
 try:v=review(a.capsule)
 except (OSError,ValueError,json.JSONDecodeError) as e:p.error(str(e))
 a.output.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
