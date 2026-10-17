"""Closed seven-source raw acquisition; never patch Path/import or run a tool."""
from pathlib import Path
import hashlib,json
DATA_SHA='61af497922931df374510a7d8acd4b8691481f960407f1670c4d4e4dc37625fe'
KEYS=('src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs', 'src/cleanup_plan/replay/factored/canonical_shared/tests.rs', 'src/cleanup_plan/replay/sequence_prefix.rs', 'src/cleanup_plan/replay/sequence_vector/funded_tests.rs', 'src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs', 'src/workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs')
def sha(raw):return hashlib.sha256(raw).hexdigest()
def unique(pairs):
 result={}
 for k,v in pairs:
  assert k not in result,'duplicate source metadata field'
  result[k]=v
 return result
def records(repo):
 raw=(Path(repo)/'docs/evidence/compile-acquisition-35f-20261017/sources.json').read_bytes()
 assert len(raw)<=1_048_576 and sha(raw)==DATA_SHA,'complete sealed compile table'
 rows=json.loads(raw,object_pairs_hook=unique)
 assert tuple(r['source']for r in rows)==KEYS
 return rows
def stage(repo,source,raw):
 if source not in KEYS:return raw
 repo=Path(repo);row=next(r for r in records(repo)if r['source']==source)
 actual=(repo/source).read_bytes();assert sha(actual)==row['current_sha256'],'actual new raw compile source'
 before=(repo/row['before_archive']).read_bytes();assert sha(before)==row['before_sha256']
 if sha(raw)==row['before_sha256']:
  assert raw==before
  return raw
 assert sha(raw)==row['current_sha256'],'whole compile input'
 lines=raw.decode().splitlines(keepends=True);last=len(lines)
 for e in reversed(row['inverse']):
  a,z=e['start'],e['end'];assert 0<=a<=z<=last and z<=len(lines)
  assert ''.join(lines[a:z])==e['current'];lines[a:z]=e['previous'].splitlines(keepends=True);last=a
 restored=''.join(lines).encode();assert sha(restored)==row['before_sha256'] and restored==before
 return restored
