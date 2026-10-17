"""Exact outer view for three raw carriers; does not execute Rust."""
from pathlib import Path
import hashlib,json
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
LAYER=HERE/'outer-carrier-final'
sha=lambda raw:hashlib.sha256(raw).hexdigest()
def unique(pairs):
 result={}
 for key,value in pairs:
  assert key not in result,('duplicate JSON field',key)
  result[key]=value
 return result
raw=(LAYER/'cuts.json').read_bytes()
assert sha(raw)=='eacf0ce6d065e55124fb3aab851560eaa4bd935cf716b3d420e6bba91fb91248'
DATA=json.loads(raw,object_pairs_hook=unique)
assert set(DATA)=={'schema','sealed_head','snapshot_head','snapshot_tree','owners','rows'}
assert DATA['schema']=='semaprax.codec-outer-three-carrier.v1' and DATA['sealed_head']=='87821ac6f304ca7ed69d5a539354e31e5db22845'
assert [row['path']for row in DATA['rows']]==['tests/hir_module_boundaries.rs', 'tests/workspace/semantic_graph/source_inventory.rs', 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
ROWS={row['path']:row for row in DATA['rows']}
def restore(name,current):
 row=ROWS[name]
 assert set(row)=={'path','current_sha256','sealed_sha256','current','sealed','edits'}
 assert sha(current.encode())==row['current_sha256'],(name,'outer whole current drift')
 assert current.encode()==(LAYER/row['current']).read_bytes(),(name,'outer current preimage drift')
 text=current;last=len(text)
 for edit in reversed(row['edits']):
  assert set(edit)=={'start','end','after','before'}
  start,end=edit['start'],edit['end']
  assert type(start)is int and type(end)is int and 0<=start<=end<=last<=len(text)
  assert text[start:end]==edit['after'],(name,'outer inverse endpoint drift')
  text=text[:start]+edit['before']+text[end:];last=start
 assert sha(text.encode())==row['sealed_sha256'],(name,'outer whole sealed drift')
 assert text.encode()==(LAYER/row['sealed']).read_bytes(),(name,'outer sealed preimage drift')
 return text
def read_input(path):
 path=Path(path);name=path.resolve().relative_to(ROOT).as_posix()
 current=path.read_text()
 return restore(name,current)if name in ROWS else current
for name in ROWS:read_input(ROOT/name)
