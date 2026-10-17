"""Finite strict successor; original algorithm and historical maximums remain intact."""
from pathlib import Path
import hashlib,json,types,re,subprocess,os,stat
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
G=R/'docs/evidence/main043-independent-custody-20261016'
sha=lambda b:hashlib.sha256(b).hexdigest()
ORIGINAL_SHA='ccfb62c8b30e5564446ddc6eb2a31f5a7502e5ed43d3d5077a8c0105c42ef95a'
ALLOWED_SHA='0969a237be8a596d0b469f8da25aa5e99f88c6190b46c263abf69f048a14e5a7'
SCRATCH=('src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch.rs','src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs')
CARRIER='tests/workspace/semantic_graph/source_inventory.rs'
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
BINDINGS={'crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.json': [14747, '2bc1821ded60594e5c2ea2a4b0553e36975776dfb43c7f195c9b49689eaee060'], 'tests__hir_module_boundaries.rs.json': [12399, 'a99febda8d720446292b504dfa7cd9af95fb40e2705f88580b42be78a5c19d63'], 'tests__workspace__semantic_graph__source_inventory.rs.json': [17315, 'a98297ee4d1f161e0484680c22168253b86050243845d168e02b7a55f6b4935b']}
def accepted_guard_extensions(root):
 assert Path(root).resolve()==R
 raw=bounded(G/'guard-extensions.py',5113,ORIGINAL_SHA)
 assert raw==bounded(D/'before/guard-extensions.py',5113,ORIGINAL_SHA)
 bounded(D/'current-allowed.json',70162,ALLOWED_SHA)
 edits=[("(D/'guard-extension-allowed.json').read_text()",'_accepted_allowed_text()'), ("D/'guard-extension-bindings'", "Path("+repr(str(D/'current-bindings'))+")"), ('json.loads(target.read_text())','json.loads(_accepted_binding_text(target))') ]
 current=raw.decode()
 for old,new in edits:
  assert current.count(old)==(1 if old=='json.loads(target.read_text())' else 2)
  current=current.replace(old,new)
 restored=current
 for old,new in reversed(edits):restored=restored.replace(new,old)
 assert restored.encode()==raw
 def allowed_text():return bounded(D/'current-allowed.json',70162,ALLOWED_SHA).decode()
 def binding_text(path):
  assert path.parent==D/'current-bindings' and path.name in BINDINGS
  length,digest=BINDINGS[path.name];return bounded(path,length,digest).decode()
 m=types.ModuleType('accepted_strict_guard');m.__file__=str(G/'guard-extensions.py')
 m.__dict__.update(_accepted_allowed_text=allowed_text,_accepted_binding_text=binding_text)
 exec(compile(current,m.__file__,'exec'),m.__dict__)
 original_verify=m.verify_guard
 def verify_guard(path,current,original,refresh=False):
  if path==CARRIER:
   # Separate indivisible lexical macro roles, not a total-count relaxation.
   core=current.split('macro_rules! semantic_graph_private_layout_owned_source_inventory {',1)[1].split('(core) => {',1)[1].split('macro_rules! semantic_graph_private_layout_source_inventory {',1)[0]
   root=current.split('macro_rules! semantic_graph_root_source_inventory {',1)[1].split('macro_rules!',1)[0]
   for name in SCRATCH:
    literal='include_str!("../../'+name+'")'
    assert current.count(literal)==2 and core.count(literal)==1 and root.count(literal)==1,('scratch role drift',name)
  return original_verify(path,current,original,refresh)
 m.verify_guard=verify_guard
 return m.__dict__
