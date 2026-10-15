from pathlib import Path
import json,hashlib
R=Path.cwd();D=R/'docs/evidence/compile-correction-e3eb-20261015'
sha=lambda b:hashlib.sha256(b).hexdigest()
count=0
for name in ['source-inverses.json','pending-binding-adapters.json']:
 for row in json.loads((D/name).read_text()):
  current=(R/row['source']).read_bytes();original=(R/row['archive']).read_bytes()
  assert sha(current)==row['current_sha256'] and sha(original)==row['original_sha256']
  text=current.decode()
  for edit in row['edits']:
   assert text.count(edit['new'])==1;text=text.replace(edit['new'],edit['before'],1)
  assert text.encode()==original
  for transition in row.get('authenticated_source_transitions',[]):
   assert sha((R/transition['archive']).read_bytes())==transition['prior_sha256']
   assert sha((R/transition['source']).read_bytes())==transition['current_sha256']
  count+=1
print('PASS complete immediate source/pending inverses:',count)
