#!/usr/bin/env python3
"""Offline authenticate the nine-pair continuation of the Boolean agent campaign."""
import argparse,hashlib,importlib.util,json,pathlib
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location('pilot',ROOT/'law16_boolean_negation_agent_pilot.py');P=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(P)
SCHEMA='semaprax.bend2-law-benchmark.boolean-negation-agent-campaign-review.v1'
def ref(root,path):
 b=path.read_bytes();return {'path':path.relative_to(root).as_posix(),'bytes':len(b),'sha256':'sha256:'+hashlib.sha256(b).hexdigest()}
def row(root,ordinal,lane):
 base=root/f'ordinal-{ordinal}'; record=base/f'{lane}.json'; trial=f'boolean-negation-pair-v1:{"bend2" if lane=="bend" else "semaprax-scalar-v1"}:{ordinal}'; language='bend2' if lane=='bend' else 'semaprax-scalar-v1'
 value,_=P.agent(root,ref(root,record),trial,language)
 if lane=='bend':
  replay=base/'bend-replay'; contract={'candidate_source':ref(root,base/'bend/final-source.bend'),'attack_source':ref(root,replay/'attack-source.bend')}
 else:
  replay=base/'semaprax-replay';contract={'candidate_source':ref(root,replay/'candidate/src/app.spx'),'attack_source':ref(root,replay/'attack/src/app.spx')}
 contract|={'candidate':{'exit_code':0,'stdout':ref(root,replay/'candidate.stdout'),'stderr':ref(root,replay/'candidate.stderr')},'attack':{'exit_code':1,'stdout':ref(root,replay/'attack.stdout'),'stderr':ref(root,replay/'attack.stderr')}}
 source,attack=P.route(root,contract,language)
 model=base/lane/('final-source.bend' if lane=='bend' else 'final-source.spx')
 if source.read_bytes()!=model.read_bytes():raise ValueError('evaluated source differs from retained model final source')
 expected=ROOT/'fixtures'/('bend-boolean-negation-law-gaming-v1.bend' if lane=='bend' else 'semaprax-boolean-negation-law-gaming-v1.spx')
 if attack.read_bytes()!=expected.read_bytes():raise ValueError('replay attack differs from fixed fixture')
 return {'trial_id':trial,'chargeable_tokens':value['telemetry']['token_usage']['total_tokens'],'cached_input_tokens':value['telemetry']['token_usage']['cached_input_tokens'],'candidate':'accepted','attack':'rejected'}
def review(root):
 root=root.resolve(strict=True);summary=json.loads((root/'summary.json').read_text())
 if summary.get('schema')!='semaprax.bend2-law-benchmark.boolean-negation-agent-campaign.v1' or summary.get('status')!='completed_replayed' or summary.get('ordinals')!=list(range(2,11)):raise ValueError('campaign summary identity drifted')
 rows=[]
 for ordinal in range(2,11): rows.append({'ordinal':ordinal,'bend2':row(root,ordinal,'bend'),'semaprax-scalar-v1':row(root,ordinal,'semaprax')})
 return {'schema':SCHEMA,'status':'nine_matched_pairs_authenticated','ordinals':list(range(2,11)),'pairs':rows,'aggregate':{'pairs':9,'bend_candidate_acceptances':9,'bend_attack_rejections':9,'semaprax_candidate_discharges':9,'semaprax_attack_rejections':9},'cost_usage':{'status':'unavailable','reason':'codex_exec_json_events_do_not_supply_monetary_usage'},'nonclaims':['ordinal 1 is retained in a separate authenticated pilot capsule','no cross-route timing ratio or winner','source proof does not prove lowering or execution']}
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--capsule',required=True,type=pathlib.Path);p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args(argv)
 if a.output.exists() or not a.output.parent.is_dir():p.error('output must be new')
 try:v=review(a.capsule)
 except (OSError,ValueError,json.JSONDecodeError,KeyError) as e:p.error(str(e))
 a.output.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
