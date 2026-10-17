from pathlib import Path
import hashlib,json,os,stat
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
def bounded(path,length,digest):
 assert 0<=length<=262144 and hasattr(os,'O_NOFOLLOW')
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode) and first.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as stream:
   raw=stream.read(length);assert len(raw)==length and not stream.read(1)
  after=os.fstat(fd);physical=os.lstat(path)
  assert stat.S_ISREG(physical.st_mode) and identity(first)==identity(after)==identity(physical)
  assert hashlib.sha256(raw).hexdigest()==digest
  raw.decode('utf8');return raw
 finally:os.close(fd)


CARRIER='tests/workspace/semantic_graph/source_inventory.rs'
SCRATCH=('src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch.rs','src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs')
def earned_owner(root):
 assert Path(root).resolve()==R
 stage=json.loads(bounded(D/'earned-source-stage.json',5215,'b23a0a3d3c62f945da767000958cbc37cb9d18d3032a65b87903b5895a52bc6d'))
 assert stage['producer_head']=='ef788f8237beeb16dcc1cfa9346fc1dbb6f69a4d' and len(stage['rows'])==2
 assert {row['source']for row in stage['rows']}==set(('tests/workspace/semantic_graph.rs', 'src/workspace_graph/type_proof_tests.rs'))
 for row in stage['rows']:
  current=row['current'];prior=row['prior']
  actual=bounded(R/row['source'],current['length'],current['sha256'])
  assert actual==bounded(R/current['archive'],current['length'],current['sha256'])
  before=bounded(R/prior['archive'],prior['length'],prior['sha256'])
  restored=actual
  for edit in reversed(row['edits']):
   after=edit['after'].encode();previous=edit['before'].encode();assert restored.count(after)==1
   restored=restored.replace(after,previous,1)
  assert restored==before
  if row['source']=='tests/workspace/semantic_graph.rs':owner=before
 return owner

def accepted_guard_extensions(root):
 assert Path(root).resolve()==R
 raw=bounded(R/'docs/evidence/compile3-second-borrow-source-acquisition-20261017/guard_payload.py',3084,'d639966e45394029dffa61d03136c5fc3648b278b9a61643303d6d48675874e5')
 assert raw==bounded(D/'guard.before.py.txt',3084,'d639966e45394029dffa61d03136c5fc3648b278b9a61643303d6d48675874e5')
 old="bounded(R/'tests/workspace/semantic_graph.rs',57424,'b291ce67ab1eb43ca4407c85b53b157d4d76f4ffb6c7091c63d91fb8f641ae9c')";new='_earned_owner(R)'
 source=raw.decode();assert source.count(old)==1
 current=source.replace(old,new,1);assert current.replace(new,old,1).encode()==raw
 ns={'__file__':str(R/'docs/evidence/compile3-second-borrow-source-acquisition-20261017/guard_payload.py'),'_earned_owner':earned_owner};exec(compile(current,ns['__file__'],'exec'),ns)
 return ns['accepted_guard_extensions'](R)
