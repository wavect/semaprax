#!/usr/bin/env python3
"""Verify matched Boolean-negation peak-RSS sample files without comparing routes."""
import argparse,hashlib,json,pathlib,statistics
ROOT=pathlib.Path(__file__).parent;SCHEMA='semaprax.bend2-law-benchmark.boolean-negation-peak-rss-review.v1';CURRENT_SCHEMA='semaprax.bend2-law-benchmark.boolean-negation-peak-rss-current.v2'
def digest(p):return 'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
def route(root,name,input_path,recorded_command=None):
 v=json.loads((root/name).read_text()); rows=v.get('samples')
 if v.get('schema')!='semaprax.bend2-law-benchmark.peak-rss.v1' or not isinstance(rows,list) or len(rows)!=30:raise ValueError(f'{name} lacks 30 samples')
 rss=[]
 for row in rows:
  if row.get('exit_code')!=0 or row.get('wrapper')!=['/usr/bin/time','-l'] or not isinstance(row.get('peak_rss_bytes'),int) or row['peak_rss_bytes']<=0:raise ValueError(f'{name} has invalid sample')
  rss.append(row['peak_rss_bytes'])
 if recorded_command is None:
  if any(str(input_path) not in row.get('command',[]) for row in rows):raise ValueError(f'{name} command does not bind retained input')
 elif any(row.get('command')!=recorded_command for row in rows):raise ValueError(f'{name} command differs from bound provenance')
 ordered=sorted(rss);return {'samples':30,'p50_peak_rss_bytes':statistics.median(rss),'p95_peak_rss_bytes':ordered[28],'input_sha256':digest(input_path),'wrapper':'/usr/bin/time -l'}
def review(root):
 root=root.resolve();bend=root/'bend-input.bend';sem=root/'semaprax-project/src/app.spx'
 if bend.read_bytes()!=(ROOT/'fixtures/bend-boolean-negation-v1.bend').read_bytes() or sem.read_bytes()!=(ROOT/'fixtures/semaprax-boolean-negation-v1.spx').read_bytes():raise ValueError('retained RSS source differs from matched fixture')
 return {'schema':SCHEMA,'status':'local_matched_rss_authenticated','routes':{'bend_verdict':route(root,'bend-verdict.json',bend),'semaprax_z3':route(root,'semaprax-z3.json',root/'semaprax-project/semaprax.toml')},'nonclaims':['RSS includes /usr/bin/time wrapper observation','no RSS ratio or winner','distinct trusted computing bases','not cold-cache isolation']}
def review_current(root):
 root=root.resolve();manifest=json.loads((root/'manifest.json').read_text())
 if manifest.get('schema')!=CURRENT_SCHEMA or manifest.get('status')!='completed' or manifest.get('samples_per_route')!=30:raise ValueError('current RSS manifest drifted')
 bend=root/manifest['bend_input']['path'];source=root/manifest['semaprax_source']['path'];project=root/manifest['semaprax_manifest']['path']
 for ref,path in ((manifest['bend_input'],bend),(manifest['semaprax_source'],source),(manifest['semaprax_manifest'],project),(manifest['provenance'],root/'provenance.json'),(manifest['bend_receipt'],root/'bend-verdict.json'),(manifest['semaprax_receipt'],root/'semaprax-z3.json')):
  if path.stat().st_size!=ref['bytes'] or digest(path)!=ref['sha256']:raise ValueError('current RSS identity drifted')
 if bend.read_bytes()!=(ROOT/'fixtures/bend-boolean-negation-v1.bend').read_bytes() or source.read_bytes()!=(ROOT/'fixtures/semaprax-boolean-negation-v1.spx').read_bytes():raise ValueError('current RSS source differs from matched fixture')
 provenance=json.loads((root/'provenance.json').read_text())
 if provenance.get('schema')!=CURRENT_SCHEMA or provenance.get('status')!='observed_current_checkout' or provenance.get('cold_cache',{}).get('status')!='unavailable':raise ValueError('current RSS provenance drifted')
 commands=provenance.get('commands',{});tools=provenance.get('tools',{});bend_argv=commands.get('bend_verdict',[]);sem_argv=commands.get('semaprax_z3',[])
 if len(bend_argv)!=4 or bend_argv[-1]!='--verdict' or pathlib.Path(bend_argv[2]).name!='bend-input.bend':raise ValueError('current RSS Bend command drifted')
 captured_root=pathlib.Path(bend_argv[2]).parent
 expected_bend=[tools['bun']['identity']['path'],tools['bend_main']['identity']['path'],str(captured_root/'bend-input.bend'),'--verdict']
 expected_sem=[tools['semaprax']['identity']['path'],'project-proof-check',str(captured_root/'semaprax-project/semaprax.toml'),'--tool','z3','--executable',tools['z3']['identity']['path'],'--version-line','Z3 version 4.12.5 - 64 bit','--host-profile','trusted-local','--source','src/app.spx','--declaration','app.negate','--ensures','0']
 if bend_argv!=expected_bend or sem_argv!=expected_sem:raise ValueError('current RSS command does not bind captured tools and inputs')
 return {'schema':SCHEMA,'status':'current_checkout_matched_rss_authenticated','routes':{'bend_verdict':route(root,'bend-verdict.json',bend,bend_argv),'semaprax_z3':route(root,'semaprax-z3.json',project,sem_argv)},'provenance':manifest['provenance'],'nonclaims':['RSS includes /usr/bin/time wrapper observation','no RSS ratio or winner','distinct trusted computing bases','not cold-cache isolation','does not rebind historical RSS evidence']}
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--capsule',required=True,type=pathlib.Path);p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args(argv)
 if a.output.exists() or not a.output.parent.is_dir():p.error('output must be new')
 try:v=review(a.capsule)
 except (OSError,ValueError,json.JSONDecodeError) as e:p.error(str(e))
 a.output.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
