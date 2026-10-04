#!/usr/bin/env python3
"""Authenticate one raw matched Boolean-negation agent pilot without invoking tools."""
import argparse, hashlib, importlib.util, json, pathlib, stat
ROOT=pathlib.Path(__file__).parent
PLAN=ROOT/'evidence/law16-boolean-negation-agent-plan-v1.json'
SCHEMA='semaprax.bend2-law-benchmark.boolean-negation-agent-pilot.v1'
REVIEW_SCHEMA='semaprax.bend2-law-benchmark.boolean-negation-agent-pilot-review.v1'

def digest(data): return 'sha256:'+hashlib.sha256(data).hexdigest()
def file(root,ref):
 if not isinstance(ref,dict) or set(ref)!={'path','bytes','sha256'}: raise ValueError('malformed artifact reference')
 p=pathlib.PurePosixPath(ref['path'])
 if p.is_absolute() or any(x in ('','.','..') for x in p.parts): raise ValueError('unsafe artifact path')
 q=root
 for x in p.parts:
  q=q/x
  if stat.S_ISLNK(q.lstat().st_mode): raise ValueError('artifact link')
 if not q.is_file() or q.stat().st_size!=ref['bytes'] or digest(q.read_bytes())!=ref['sha256']: raise ValueError('artifact digest drifted')
 return q
def agent(root,reference,expected_id,language):
 p=file(root,reference); value=json.loads(p.read_text())
 if value.get('status')!='edit_artifacts_captured' or value.get('trial')!={'id':expected_id,'language':language}: raise ValueError('agent record is not a captured selected trial')
 usage=value.get('telemetry',{}).get('token_usage')
 if not isinstance(usage,dict) or usage.get('total_tokens')!=usage.get('input_tokens',-1)+usage.get('output_tokens',-1) or usage['total_tokens']>20000: raise ValueError('agent token budget drifted')
 if value.get('telemetry',{}).get('cost_usage',{}).get('status')!='unavailable': raise ValueError('unexpected cost claim')
 ed=p.parent / p.stem
 artifacts=value.get('artifacts',{}); edit=value.get('edit_artifacts',{})
 for row in (*artifacts.values(),*edit.values()): file(ed,row)
 events=[json.loads(x) for x in (ed/artifacts['events']['path']).read_text().splitlines() if x]
 complete=[x for x in events if x.get('type')=='turn.completed']
 if len(complete)!=1 or complete[0].get('usage') is None: raise ValueError('agent raw event usage drifted')
 raw=complete[0]['usage']
 if {k:raw.get(k) for k in ('input_tokens','cached_input_tokens','output_tokens')}!={k:usage.get(k) for k in ('input_tokens','cached_input_tokens','output_tokens')}: raise ValueError('agent telemetry differs from raw event')
 return value,ed
def route(root,row,lane):
 source=file(root,row['candidate_source']); attack=file(root,row['attack_source'])
 candidate=row['candidate']; attack_row=row['attack']
 for check in (candidate,attack_row): file(root,check['stdout']);file(root,check['stderr'])
 cstdout=(root/candidate['stdout']['path']).read_bytes(); cstderr=(root/candidate['stderr']['path']).read_bytes(); astderr=(root/attack_row['stderr']['path']).read_bytes()
 if lane=='bend2':
  if candidate['exit_code']!=0 or b'ALL PROOFS CHECK' not in cstdout or attack_row['exit_code']==0 or b'SOME PROOFS FAIL' not in astderr: raise ValueError('Bend candidate or control route drifted')
 else:
  if candidate['exit_code']!=0 or cstderr or attack_row['exit_code']==0 or b'SPX-LW140' not in astderr: raise ValueError('SEMAPRAX candidate or control route drifted')
  output=json.loads(cstdout); obligations=output['project_assurance']['payload']['obligations']
  if not any(o.get('declaration_id')=='app.negate' and o.get('classification')=='smt_proved' and any(m.get('tool')=='z3' and m.get('class')=='smt_proved' for m in o.get('methods',[])) for o in obligations): raise ValueError('SEMAPRAX candidate lacks Z3 discharge')
 return source,attack
def review(root):
 root=root.resolve(strict=True); pilot=json.loads((root/'pilot.json').read_text())
 if pilot.get('schema')!=SCHEMA or pilot.get('status')!='candidate_and_attack_routes_observed' or pilot.get('ordinal')!=1: raise ValueError('pilot identity drifted')
 if pilot.get('plan',{}).get('sha256')!=digest(PLAN.read_bytes()): raise ValueError('pilot plan digest drifted')
 lanes=pilot.get('lanes',{}); result={}
 expected={'bend2':('boolean-negation-pair-v1:bend2:1',ROOT/'fixtures/bend-boolean-negation-law-gaming-v1.bend'),'semaprax-scalar-v1':('boolean-negation-pair-v1:semaprax-scalar-v1:1',ROOT/'fixtures/semaprax-boolean-negation-law-gaming-v1.spx')}
 for lane,(trial,attack_fixture) in expected.items():
  record,ed=agent(root,lanes[lane]['agent_record'],trial,lane); source,attack=route(root,lanes[lane],lane)
  model_source=ed/record['edit_artifacts']['final_source']['path']
  if source.read_bytes()!=model_source.read_bytes(): raise ValueError('evaluated source differs from model final source')
  if attack.read_bytes()!=attack_fixture.read_bytes(): raise ValueError('evaluated attack differs from fixed pair control')
  result[lane]={'trial_id':trial,'chargeable_tokens':record['telemetry']['token_usage']['total_tokens'],'cached_input_tokens':record['telemetry']['token_usage']['cached_input_tokens'],'cost_usage':record['telemetry']['cost_usage'],'candidate':'accepted by retained pinned route','attack':'rejected by retained pinned route'}
 return {'schema':REVIEW_SCHEMA,'status':'one_matched_pilot_authenticated','ordinal':1,'lanes':result,'cost_usage':pilot['cost_usage'],'nonclaims':pilot['nonclaims']}
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--capsule',required=True,type=pathlib.Path);p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args(argv)
 if a.output.exists() or not a.output.parent.is_dir():p.error('output must be new')
 try: value=review(a.capsule)
 except (OSError,ValueError,json.JSONDecodeError,KeyError) as e:p.error(str(e))
 a.output.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
