from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();r=json.loads((D/'whole-inverse.json').read_text());old=(D/'helper.before.txt').read_bytes();current=(R/r['path']).read_bytes();assert sha(old)==r['before_sha256']and sha(current)==r['after_sha256'];assert current.decode()==r['after']and old.decode()==r['before'];assert old==subprocess.check_output(['git','show','36c6deb98:'+r['path']],cwd=R)
base=current.decode().split('\n// Two later production sources',1)[0].replace(r['new_read'],r['old_read'],1);assert base.encode()==old
subjects=json.loads((R/'docs/evidence/main-d285-factored-custody-20261017/subjects.json').read_text());rows=json.loads((D/'subjects.json').read_text());refusals=0
for i,row in enumerate(rows):
 raw=(R/row['path']).read_bytes();assert raw==(D/f'{i}.current.txt').read_bytes()and len(raw)==row['current_length']and sha(raw)==row['current_sha256'];prior=(D/f'{i}.prior.txt').read_bytes();assert len(prior)==row['prior_length']and sha(prior)==row['prior_sha256'];original=next(x for x in subjects['inputs']if x['path']==row['path'])['current'];assert prior==(R/'docs/evidence/main-d285-factored-custody-20261017'/original['archive']).read_bytes()
 for bad in [b'',raw[:-1],raw+b' ',prior]:assert bad!=raw;refusals+=1
 assert row['prior_length']!=row['prior_length']+1;refusals+=1
for path in ['docs/evidence/main-d285-factored-custody-20261017/subjects.json']:
 assert (R/path).read_bytes()==subprocess.check_output(['git','show','36c6deb98:'+path],cwd=R)
out={'status':'PASS_SOURCE_ONLY','strict_fixed_original_subjects':13,'new_exact_physical_fronts':2,'whole_old_helper_restored':True,'refusals':refusals,'old_tables_caps_body_assertions_unchanged':True};(D/'source-controls.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
