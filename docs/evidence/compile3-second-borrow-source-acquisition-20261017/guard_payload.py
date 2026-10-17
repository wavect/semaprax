"""One exact whole-owner acquisition; all prior strict guard bodies stay intact."""
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
def _compile3_owner(root):
 assert Path(root).resolve()==R
 actual=bounded(R/'tests/workspace/semantic_graph.rs',57424,'b291ce67ab1eb43ca4407c85b53b157d4d76f4ffb6c7091c63d91fb8f641ae9c')
 assert actual==bounded(D/'owner.after.rs.txt',57424,'b291ce67ab1eb43ca4407c85b53b157d4d76f4ffb6c7091c63d91fb8f641ae9c')
 before=bounded(D/'owner.before.rs.txt',57423,'57ea6ed78a86d14bf977425ec48dc9d68b346eba03f64724754c49dc1088e070')
 return owner_previous(actual,before)

def owner_previous(actual,before):
 assert (len(actual),hashlib.sha256(actual).hexdigest())==(57424,'b291ce67ab1eb43ca4407c85b53b157d4d76f4ffb6c7091c63d91fb8f641ae9c')
 assert (len(before),hashlib.sha256(before).hexdigest())==(57423,'57ea6ed78a86d14bf977425ec48dc9d68b346eba03f64724754c49dc1088e070')
 old='        source_inventory_controls::current_owned_source_inventory(),';new='        &source_inventory_controls::current_owned_source_inventory(),'
 assert before.count(old.encode())==actual.count(new.encode())==1
 assert actual.replace(new.encode(),old.encode(),1)==before
 return before

def accepted_guard_extensions(root):
 assert Path(root).resolve()==R
 raw=bounded(R/'docs/evidence/workspace21-legacy-source-repair-20261017/guard_payload.py',5028,'5ba509e7343ac8ada5b6dd4f7e7ab22df4856d28f455d0b902c088b896cdad35')
 assert raw==bounded(D/'guard.before.py.txt',5028,'5ba509e7343ac8ada5b6dd4f7e7ab22df4856d28f455d0b902c088b896cdad35')
 old="bounded(R/'tests/workspace/semantic_graph.rs',57423,'57ea6ed78a86d14bf977425ec48dc9d68b346eba03f64724754c49dc1088e070')";new='_compile3_owner(R)'
 source=raw.decode();assert source.count(old)==1
 current=source.replace(old,new,1);assert current.replace(new,old,1).encode()==raw
 ns={'__file__':str(R/'docs/evidence/workspace21-legacy-source-repair-20261017/guard_payload.py'),'_compile3_owner':_compile3_owner}
 exec(compile(current,ns['__file__'],'exec'),ns)
 return ns['accepted_guard_extensions'](R)
