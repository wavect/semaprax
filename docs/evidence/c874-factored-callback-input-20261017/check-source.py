"""Read-only full callback/source-domain composition, not Rust execution."""
from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'inputs.json').read_bytes());assert len(rows)==3
old=json.loads((R/'docs/evidence/mainbe8-flat-targets-joined-20261017/joined-inputs.json').read_bytes())
S=R/'docs/evidence/main-d285-factored-custody-20261017';subjects=json.loads((S/'subjects.json').read_bytes())['inputs']
def get(path):return (S/next(x for x in subjects if x['path']==path)['ours']['archive']).read_bytes()
def archive(row):
 b=(D/row['archive']).read_bytes();assert len(b)==row['length'] and h(b)==row['sha256'];assert len(b)<=2097152;return b
proof=json.loads(get('docs/evidence/factored-flat-targets-v1/sources.json'))
comp=json.loads((R/'docs/evidence/compile-acquisition-35f-20261017/sources.json').read_bytes())
negative=0
for record in proof['predecessors']:
 path=record['path'];row=next(x for x in rows if x['path']==path)
 # Actual relocated OURS read plus original compile35f acquisition.
 stage=get(path);cr=next((x for x in comp if x['source']==path),None)
 if cr:
  assert h(stage)==cr['current_sha256'];stage=(R/cr['before_archive']).read_bytes();assert h(stage)==cr['before_sha256']
 assert stage==archive(row['stage'])
 joined=archive(row['joined']);assert joined==subprocess.check_output(['git','show','51c1dfc49962e046566ad21ec31f20c6b3a9bdae:'+path],cwd=R)
 original=next(x for x in old if x['path']==path);assert h(joined)==original['current_sha256']
 text=joined.decode()
 for edit in reversed(original['edits']):
  assert text[edit['start']:edit['end']]==edit['current'];text=text[:edit['start']]+edit['incoming']+text[edit['end']:]
 callback=text.encode();assert callback==archive(row['joined_result'])==(R/original['incoming_archive']).read_bytes();assert h(callback)==original['incoming_sha256']
 # The separate OURS afterimage is independently reconstructed using ALL old
 # inverse rows from the full original before, then checked in BOTH directions.
 before=get('docs/evidence/factored-flat-targets-v1/'+record['archive']);assert h(before)==record['before_sha256'];text=before.decode()
 for edit in reversed(record['inverse']):
  assert text.count(edit['before'])==1;text=text.replace(edit['before'],edit['after'],1)
 after=text.encode();assert after==archive(row['ours_after']);assert h(after)==record['after_sha256']
 for edit in record['inverse']:
  assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
 assert text.encode()==before
 for bad in [b'',stage[:-1],stage+b' ',stage+stage,callback]:
  assert bad!=stage and bad!=joined;negative+=1
 # Original drift/hostile source assertions are retained and remain nonmatching.
 for bad in [after+b'\n',b'!'+after[1:]]:
  text=bad.decode();ok=True
  for edit in record['inverse']:
   if text.count(edit['after'])!=1:ok=False;break
   text=text.replace(edit['after'],edit['before'],1)
  assert not ok or text.encode()!=before;negative+=1
for record in proof['new_files']:
 b=get(record['path']);cr=next((x for x in comp if x['source']==record['path']),None)
 if cr:b=(R/cr['before_archive']).read_bytes()
 assert h(b)==record['sha256']
p='src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs';inv=json.loads((D/'whole-source-inverse.json').read_bytes());now=(R/p).read_bytes();assert h(now)==inv['after_sha256'];assert now.decode()==inv['inverse'][0]['after'];before=inv['inverse'][0]['before'].encode();assert before==(D/'incoming-flat-targets.before.rs.txt').read_bytes()==subprocess.check_output(['git','show','c8740045e33126a4375aadd1dc524a25be9245fc:'+p],cwd=R);assert h(before)==inv['before_sha256']
assert len(now.splitlines())<1500
print(json.dumps({'state':'PASS_SOURCE_ONLY','all_actual_ours_predecessor_callback_chains':len(proof['predecessors']),'new_files':len(proof['new_files']),'original_callback_rows_and_result_preserved':3,'complete_ours_afterimages_reconstructed_both_directions':3,'source_mutations':negative,'original_table_unchanged':True,'rust_runtime':'UNEXECUTED'}))
