#!/usr/bin/env python3
"""Execute/replay selected preregistered Boolean-negation agent ordinals sequentially."""
import argparse, json, pathlib, shutil, subprocess, sys
ROOT=pathlib.Path(__file__).parent
PLAN=ROOT/'evidence/law16-boolean-negation-agent-plan-v1.json'
BEND_ROOT=pathlib.Path('/tmp/bend2-law-source'); BUN=pathlib.Path('/Users/kevin/.bun/bin/bun'); SEM=pathlib.Path('/tmp/semaprax-9a9db7a81'); Z3=pathlib.Path('/opt/homebrew/bin/z3')
def run(argv, cwd=None): return subprocess.run(argv,capture_output=True,cwd=cwd,timeout=620)
def write(path,data): path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
def checked_record(path,trial):
 value=json.loads(path.read_text()); usage=value.get('telemetry',{}).get('token_usage',{})
 if value.get('status')!='edit_artifacts_captured' or value.get('trial',{}).get('id')!=trial or not value.get('telemetry',{}).get('within_fixed_token_budget') or value.get('telemetry',{}).get('cost_usage',{}).get('status')!='unavailable' or usage.get('total_tokens')!=usage.get('input_tokens',-1)+usage.get('output_tokens',-1): raise ValueError('agent record is ineligible or telemetry changed')
 return value
def bend(root,ordinal):
 trial=f'boolean-negation-pair-v1:bend2:{ordinal}'; out=root/f'ordinal-{ordinal}/bend'; out.parent.mkdir(parents=True, exist_ok=True); record=root/f'ordinal-{ordinal}/bend.json'
 call=run([sys.executable,str(ROOT/'codex_agent_trial.py'),'--plan',str(PLAN),'--trial',trial,'--evidence-dir',str(out),'--output',str(record)])
 if call.returncode: raise ValueError(f'{trial} agent turn failed: {call.stderr.decode(errors="replace")}')
 value=checked_record(record,trial); raw=root/f'ordinal-{ordinal}/bend-replay'; raw.mkdir()
 candidate=run([str(BUN),str(BEND_ROOT/'bend2/main.ts'),str(out/'final-source.bend'),'--verdict']); attack=run([str(BUN),str(BEND_ROOT/'bend2/main.ts'),str(ROOT/'fixtures/bend-boolean-negation-law-gaming-v1.bend'),'--verdict'])
 write(raw/'candidate.stdout',candidate.stdout);write(raw/'candidate.stderr',candidate.stderr);write(raw/'attack.stdout',attack.stdout);write(raw/'attack.stderr',attack.stderr);shutil.copyfile(ROOT/'fixtures/bend-boolean-negation-law-gaming-v1.bend',raw/'attack-source.bend')
 if candidate.returncode or b'ALL PROOFS CHECK' not in candidate.stdout or attack.returncode==0 or b'SOME PROOFS FAIL' not in attack.stderr: raise ValueError(f'{trial} independent replay failed')
 return {'trial':trial,'tokens':value['telemetry']['token_usage'],'candidate_exit':candidate.returncode,'attack_exit':attack.returncode}
def sem(root,ordinal):
 trial=f'boolean-negation-pair-v1:semaprax-scalar-v1:{ordinal}'; out=root/f'ordinal-{ordinal}/semaprax'; out.parent.mkdir(parents=True, exist_ok=True); record=root/f'ordinal-{ordinal}/semaprax.json'
 call=run([sys.executable,str(ROOT/'codex_agent_trial.py'),'--plan',str(PLAN),'--trial',trial,'--evidence-dir',str(out),'--output',str(record)])
 if call.returncode: raise ValueError(f'{trial} agent turn failed: {call.stderr.decode(errors="replace")}')
 value=checked_record(record,trial); raw=root/f'ordinal-{ordinal}/semaprax-replay'; shutil.copytree(ROOT/'fixtures/boolean-negation-project-v1/candidate',raw/'candidate');shutil.copytree(ROOT/'fixtures/boolean-negation-project-v1/attack',raw/'attack'); shutil.copyfile(out/'final-source.spx',raw/'candidate/src/app.spx')
 args=lambda project:[str(SEM),'project-proof-check',str(project/'semaprax.toml'),'--tool','z3','--executable',str(Z3),'--version-line','Z3 version 4.12.5 - 64 bit','--host-profile','trusted-local','--source','src/app.spx','--declaration','app.negate','--ensures','0']
 candidate=run(args(raw/'candidate'));attack=run(args(raw/'attack'));write(raw/'candidate.stdout',candidate.stdout);write(raw/'candidate.stderr',candidate.stderr);write(raw/'attack.stdout',attack.stdout);write(raw/'attack.stderr',attack.stderr)
 try: obligations=json.loads(candidate.stdout)['project_assurance']['payload']['obligations']; discharge=any(x.get('declaration_id')=='app.negate' and x.get('classification')=='smt_proved' for x in obligations)
 except (json.JSONDecodeError,KeyError,TypeError): discharge=False
 if candidate.returncode or candidate.stderr or not discharge or attack.returncode==0 or b'SPX-LW140' not in attack.stderr: raise ValueError(f'{trial} independent replay failed')
 return {'trial':trial,'tokens':value['telemetry']['token_usage'],'candidate_exit':candidate.returncode,'attack_exit':attack.returncode}
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',required=True,type=pathlib.Path);p.add_argument('--first',type=int,default=2);p.add_argument('--last',type=int,default=10);a=p.parse_args(argv)
 if a.output.exists() or a.first<1 or a.last>10 or a.first>a.last:p.error('output must be new and ordinal range must be within 1..10')
 a.output.mkdir(parents=True); rows=[]
 try:
  for ordinal in range(a.first,a.last+1): rows.extend([bend(a.output,ordinal),sem(a.output,ordinal)])
 except (OSError,ValueError,subprocess.TimeoutExpired) as error:
  (a.output/'summary.json').write_text(json.dumps({'status':'stopped','completed':rows,'error':str(error)},indent=2)+'\n');raise SystemExit(str(error))
 (a.output/'summary.json').write_text(json.dumps({'schema':'semaprax.bend2-law-benchmark.boolean-negation-agent-campaign.v1','status':'completed_replayed','ordinals':list(range(a.first,a.last+1)),'rows':rows,'cost_usage':{'status':'unavailable','reason':'codex_exec_json_events_do_not_supply_monetary_usage'},'nonclaims':['this campaign omits ordinal 1 pilot','no cross-route ratio or winner','source proof does not prove lowering or execution']},indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
