"""Closed current receiver -> exact preexisting043 input; no compiler authority."""
import hashlib,json
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'main043-semantic-acquisition-20261017'
SOURCE='src/source_reflows/semantic_receivers.rs'
sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
def unique(pairs):
 result={}
 for key,value in pairs:
  assert key not in result,('duplicate JSON field',key)
  result[key]=value
 return result
raw=(D/'current-to-043-stage.json').read_text()
assert len(raw.encode())<=131072 and sha(raw)=='63bd9f8a4f8d19369a78798a41278e5b96ab0732361598656884efd6af159ff9'
ROW=json.loads(raw,object_pairs_hook=unique)
def restore(row,source,current):
 assert set(row)=={'source','current_sha256','previous_sha256','current','previous','inverse'}
 assert source==SOURCE==row['source']
 assert len(current.encode())<=65536 and sha(current)==row['current_sha256']
 assert current==row['current']==(D/'current.rs.txt').read_text()
 assert sha((D/'prior-stage.rs.txt').read_text())==row['previous_sha256']
 edits=row['inverse'];assert 0<len(edits)<=32
 lines=current.splitlines(keepends=True);last=len(lines)
 for edit in reversed(edits):
  assert set(edit)=={'start','end','current','previous'}
  a,z=edit['start'],edit['end'];assert type(a)is int and type(z)is int and 0<=a<=z<=last<=len(lines)
  assert ''.join(lines[a:z])==edit['current']
  lines[a:z]=edit['previous'].splitlines(keepends=True);last=a
 result=''.join(lines)
 assert sha(result)==row['previous_sha256'] and result==row['previous']==(D/'prior-stage.rs.txt').read_text()
 return result
def project(current):return restore(ROW,SOURCE,current)
