"""Closed historical runtime-json prefix; raw cap reader remains actual and exact."""
from pathlib import Path
import hashlib,json
SOURCE = "src/source_reflows_merge/runtime_json.rs"
LENGTH = 9559
SHA256 = '7718cbcff31f0bea3901415af3efd4252eed0bba5da93134b81260ccf61d0372'
BEFORE_SHA = '004c365b22682d38e0de3fa840119c5afb700547efb10dbdd2731e47641e78ad'
ORIGINAL_SHA = '987ec7791e7a04a3ecbd1149c59f379a134150709b7ee0283f37737aa6bc152e'
def sha(b):return hashlib.sha256(b).hexdigest()
def restore(row,current):
 assert row['source']==SOURCE and len(current.encode())<=65536
 assert sha(current.encode())==row['current_sha256']and current==row['current']
 edits=row['inverse'];assert len(edits)<=16
 lines=current.splitlines(keepends=True);last=len(lines)
 for e in reversed(edits):
  a,z=e['start'],e['end'];assert 0<=a<=z<=last and z<=len(lines)
  assert ''.join(lines[a:z])==e['current'];lines[a:z]=e['previous'].splitlines(keepends=True);last=a
 result=''.join(lines);assert sha(result.encode())==row['before_sha256']==BEFORE_SHA
 return result
def historical(repo,current):
 d=Path(__file__).parent/'runtime-prefix-cap-historical-view'
 payload=(d/'current-to-88cc.json').read_bytes();assert len(payload)==LENGTH<=131072 and sha(payload)==SHA256
 row=json.loads(payload);before=(d/'runtime-json-88cc-before.rs.txt').read_text();original=(d/'runtime-json-a82-original.rs.txt').read_text()
 assert sha(before.encode())==BEFORE_SHA and sha(original.encode())==ORIGINAL_SHA
 assert before.startswith(original)
 assert current==(Path(repo)/SOURCE).read_text()
 # Entire actual prefix contains OUTPUT_CAP, read_fixture, read_body and old tests registration.
 # Its literal bytes are untouched; only the later source-dispatch method is projected.
 marker='pub(super) fn maina3_input('
 assert current.count(marker)==before.count(marker)==1
 assert current.split(marker,1)[0]==before.split(marker,1)[0]
 assert 'const OUTPUT_CAP: usize = 5_081_115;' in current.split(marker,1)[0]
 assert 'maina3_part_source(&payload, source);' in current
 result=restore(row,current);assert result==before
 for bad in ['',current+' ',current+current,'!'+current[1:]]:
  try:restore(row,bad)
  except AssertionError:pass
  else:raise AssertionError('runtime source drift admitted')
 forged=dict(row);forged['before_sha256']='0'*64
 try:restore(forged,current)
 except AssertionError:pass
 else:raise AssertionError('forged historical runtime admitted')
 return result
