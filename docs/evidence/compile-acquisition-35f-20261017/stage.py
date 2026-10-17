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
 raw=_factored_core_input(repo,source,raw)
 actual=_factored_core_physical(repo,source);assert sha(actual)==row['current_sha256'],'actual new raw compile source'
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

# Exact outer core relocation only. The seven original rows and inverse are immutable.
FACTORED_CORE='src/cleanup_plan/replay/factored/canonical_shared/tests.rs'
FACTORED_CAP=2_097_152
FACTORED_RAW_LEN=30885
FACTORED_RAW_SHA='fd122ac9ba5101c7bec9046fe6b4f939a964bc362a59948b0774e64417ed3e17'
FACTORED_OURS_LEN=36646
FACTORED_OURS_SHA='c151768408bd95f3a56ae5eb48b013ac010c221d0b43df9a141ad47cd97ca953'
def _factored_file(path,length,expected):
 import os,stat
 assert 0<=length<=FACTORED_CAP,'factored bound'
 path=Path(path);before=path.lstat();assert stat.S_ISREG(before.st_mode),'factored regular source'
 flags=os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC
 fd=os.open(path,flags)
 def identity(s):return(s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_ctime_ns,s.st_mode)
 try:
  opened=os.fstat(fd);assert stat.S_ISREG(opened.st_mode)and identity(before)==identity(opened)and opened.st_size==length,'factored source identity/length'
  data=bytearray()
  while len(data)<length:
   part=os.read(fd,length-len(data));assert part,'factored truncated';data.extend(part)
  assert not os.read(fd,1),'factored extra'
  assert identity(opened)==identity(os.fstat(fd))==identity(path.lstat()),'factored drift'
 finally:os.close(fd)
 data=bytes(data);assert sha(data)==expected,'factored sha';data.decode('utf8');return data
def _factored_core_physical(repo,source):
 repo=Path(repo)
 if source!=FACTORED_CORE:return(repo/source).read_bytes()
 _factored_file(repo/source,FACTORED_RAW_LEN,FACTORED_RAW_SHA)
 return _factored_file(repo/'docs/evidence/main-d285-factored-custody-20261017/ours-core.before.txt',FACTORED_OURS_LEN,FACTORED_OURS_SHA)
def _factored_core_input(repo,source,raw):
 if source!=FACTORED_CORE:return raw
 old=_factored_core_physical(repo,source)
 if len(raw)==FACTORED_RAW_LEN and sha(raw)==FACTORED_RAW_SHA:return old
 return raw
