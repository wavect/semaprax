"""Closed Main043 historical view; never used for the actual raw source audit."""
import hashlib,json
from pathlib import Path
DATA_SHA = '1aa0092ae0b2503a0fc398efe2dd3454f9940d052f6c3e9cf5861c57e9d05b5d'
OLD_SHA = '651d174d04fad97da5c08ad215a869fd395a7d691d4a78dc901a99e814fc8252'
SOURCE = "src/source_reflows/semantic_receivers.rs"
def sha(s):return hashlib.sha256(s.encode()).hexdigest()

# Exact additional acquisition only; every original043 guard below is retained.
_outer=Path(__file__).parent/'semantic-receiver-outer.py'
assert sha(_outer.read_text())=='5ee9c8fc3272a4110bf2516b263d37697f98ea301190989645b02f0333983db4'
import runpy
_acquire=runpy.run_path(str(_outer))['project']
def restore(row,current):
 current=_acquire(current)
 assert row['source']==SOURCE and sha(current)==row['current_sha256']
 assert current==row['current'] and len(current.encode())<=65536
 edits=row['inverse'];assert len(edits)<=32
 lines=current.splitlines(keepends=True);last=len(lines)
 for e in reversed(edits):
  a,z=e['start'],e['end'];assert 0<=a<=z<=last and z<=len(lines)
  assert ''.join(lines[a:z])==e['current']
  lines[a:z]=e['previous'].splitlines(keepends=True);last=a
 restored=''.join(lines);assert sha(restored)==row['historical_sha256']==OLD_SHA
 return restored
def historical(repo,current):
 d=Path(__file__).parent/'semantic-receiver-historical-view'
 text=(d/'current-to-main043.json').read_text();assert sha(text)==DATA_SHA
 row=json.loads(text);original=(d/'main043-semantic_receivers.rs.txt').read_text()
 assert sha(original)==OLD_SHA
 result=restore(row,current);assert result==original
 # Real raw source remains the live input; this view serves only historical equality.
 assert current==(Path(repo)/SOURCE).read_text()
 for bad in ['',current+' ',current+current,'!'+current[1:]]:
  try:restore(row,bad)
  except AssertionError:pass
  else:raise AssertionError('historical input mutation admitted')
 forged=dict(row);forged['historical_sha256']='0'*64
 try:restore(forged,current)
 except AssertionError:pass
 else:raise AssertionError('forged predecessor admitted')
 return result
