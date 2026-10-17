"""Finite strict successor; original algorithm and historical maximums remain intact."""
from pathlib import Path
import hashlib,json,types,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
G=R/'docs/evidence/main043-independent-custody-20261016'
sha=lambda b:hashlib.sha256(b).hexdigest()
ORIGINAL_SHA='ccfb62c8b30e5564446ddc6eb2a31f5a7502e5ed43d3d5077a8c0105c42ef95a'
ALLOWED_SHA='0969a237be8a596d0b469f8da25aa5e99f88c6190b46c263abf69f048a14e5a7'
SCRATCH=('src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch.rs','src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs')
CARRIER='tests/workspace/semantic_graph/source_inventory.rs'
def accepted_guard_extensions(root):
 assert Path(root).resolve()==R
 raw=(G/'guard-extensions.py').read_bytes();assert sha(raw)==ORIGINAL_SHA
 assert raw==(D/'before/guard-extensions.py').read_bytes()
 assert sha((D/'current-allowed.json').read_bytes())==ALLOWED_SHA
 edits=[("D/'guard-extension-allowed.json'","Path("+repr(str(D/'current-allowed.json'))+")"), ("D/'guard-extension-bindings'","Path("+repr(str(D/'current-bindings'))+")") ]
 current=raw.decode()
 for old,new in edits:
  assert current.count(old)==2
  current=current.replace(old,new)
 restored=current
 for old,new in reversed(edits):restored=restored.replace(new,old)
 assert restored.encode()==raw
 m=types.ModuleType('accepted_strict_guard');m.__file__=str(G/'guard-extensions.py')
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
