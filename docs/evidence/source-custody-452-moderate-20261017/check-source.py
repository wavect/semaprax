from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'inputs.json').read_bytes());assert len(rows)==2
for i,r in enumerate(rows):
 for field in ['current','previous','physical']:
  raw=(D/r[field+'_archive']).read_bytes();assert len(raw)==r[field+'_length'] and sha(raw)==r[field+'_sha256']
 assert (R/r['path']).read_bytes()==(D/r['physical_archive']).read_bytes()
for i,r in enumerate(json.loads((D/'whole-inverses.json').read_text())):
 assert (R/r['path']).read_text()==r['after']and sha(r['after'].encode())==r['after_sha256'];assert (D/f'changed-{i}.before.txt').read_text()==r['before'];assert sha(r['before'].encode())==r['before_sha256']
 assert r['before'].encode()==subprocess.check_output(['git','show','7ecf72807:'+r['path']],cwd=R)
# The independently retained whole joined helper body remains, with private name only.
s=(R/'src/cleanup_plan/replay/factored/moderate_tests.rs').read_text();old=(D/'changed-1.before.txt').read_text();original='fn joined_input(path: &str, current: &str) -> String {\n    joined_checked(&joined_data(), path, current)\n        .expect("complete joined-to-incoming source domain")\n}';assert original in old and original.replace('fn joined_input(','fn joined_checkpoint_input(',1)in s
records=json.loads((R/'docs/evidence/moderate-factored-v1/sources.json').read_text());joins=json.loads((R/'docs/evidence/moderate-factored-joined-custody-20261017/joined-inputs.json').read_text());refusals=0
for rec in records:
 j=next(r for r in joins if r['path']==rec['path']);raw=(R/rec['path']).read_bytes();assert len(raw)==j['current_length']and sha(raw)==j['current_sha256'];stage=raw
 for e in reversed(j['edits']):assert stage[e['start']:e['end']]==e['current'].encode();stage=stage[:e['start']]+e['incoming'].encode()+stage[e['end']:]
 assert stage==(R/j['incoming_archive']).read_bytes();outer=next((r for r in rows if r['path']==rec['path']),None)
 if outer:
  assert sha(stage)==outer['current_sha256'];stage=(D/outer['previous_archive']).read_bytes()
  for bad in [b'',raw[:-1],stage,b'foreign']:
   assert bad!=raw;refusals+=1
  for bad in [b'',stage,(D/outer['current_archive']).read_bytes()+b' ']:assert sha(bad)!=outer['current_sha256'];refusals+=1
 text=stage.decode()
 for e in reversed(rec['edits']):assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert text==(R/'docs/evidence/moderate-factored-v1'/rec['archive']).read_text()
for path in ['docs/evidence/moderate-factored-v1/sources.json','docs/evidence/moderate-factored-joined-custody-20261017/joined-inputs.json']:
 assert (R/path).read_bytes()==subprocess.check_output(['git','show','36c6deb98:'+path],cwd=R)
out={'status':'PASS_SOURCE_ONLY','whole_moderate_original_subjects_restored':3,'new_explicit_metadata_stages':2,'refusals':refusals,'old_joined_helper_oldtables_assertions_caps_unchanged':True,'Rust_runtime':False};(D/'source-controls.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
