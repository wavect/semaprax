"""Exact ten-input compile repair acquisition before unchanged strict controls."""
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

OLD='docs/evidence/accepted-strict-guard-generation-20261017/guard_payload.py'
CARRIER='tests/workspace/semantic_graph/source_inventory.rs'
SCRATCH=('src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch.rs','src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs')
CURRENT=(96877,'4dbeb81d84db0f80bb51ccc8fc1a25d2d71e49759ddeeccdc84c4d1748b27e11')
PRIOR=(94257,'8debd7324052ca398a9385bdf7b47f1b479d8638b8e5ed9dc72ef4b9213cd177')

VIEWS={'carrier': (96877, '4dbeb81d84db0f80bb51ccc8fc1a25d2d71e49759ddeeccdc84c4d1748b27e11'), 'owner': (57423, '57ea6ed78a86d14bf977425ec48dc9d68b346eba03f64724754c49dc1088e070'), 'table': (24779, 'a5770640ed993b555705ac496ea64e6ea6786563fd9c50ba236123cf0c621280'), 'aliases': (4262, '20a27d590c5055421ed049218361d6c3f91797e6654401076e7e1972dc842946')}
def verify_repair_views(carrier,owner,table,aliases):
 for name,raw in [('carrier',carrier),('owner',owner),('table',table),('aliases',aliases)]:
  assert (len(raw),hashlib.sha256(raw).hexdigest())==VIEWS[name],('foreign repaired view',name)
 prior=bounded(R/'docs/evidence/main-c883-next-source-merge-20261017/current-projection-inputs.tsv',24016,'7612288a0b43976832ea33ebca4527d0919f1fbf25d13d9ff6733f00543eb624').decode()
 restored=table.decode()
 rows=json.loads(aliases)['rows'];assert len(rows)==10
 for row in reversed(rows):
  assert restored.splitlines().count(row['archive'])==1
  restored=restored.replace(row['archive']+'\n',row['logical']+'\n',1)
 assert restored==prior,'complete ordered prior input list changed'

def accepted_guard_extensions(root):
 assert Path(root).resolve()==R
 old=bounded(R/OLD,3965,'17d849f39380f26dcd0d12c9c3754f7480d8d3153a44a20a31d849901066e147')
 assert old==bounded(D/'before/guard_payload.py',3965,'17d849f39380f26dcd0d12c9c3754f7480d8d3153a44a20a31d849901066e147')
 inverse=json.loads(bounded(D/'carrier-inverse.json',5205,'a3fa527500a53771c7d65961eb5940042f9adb4123ccd52ec6f6a451324e6b9b'))
 legacy=json.loads(bounded(D/'legacy-inputs.json',4262,'20a27d590c5055421ed049218361d6c3f91797e6654401076e7e1972dc842946'))
 assert len(legacy['rows'])==len(inverse['edits'])==10
 verify_repair_views(bounded(R/CARRIER,*CURRENT),bounded(R/'tests/workspace/semantic_graph.rs',57423,'57ea6ed78a86d14bf977425ec48dc9d68b346eba03f64724754c49dc1088e070'),bounded(R/'docs/evidence/workspace21-legacy-source-repair-20261017/current-projection-inputs.tsv',24779,'a5770640ed993b555705ac496ea64e6ea6786563fd9c50ba236123cf0c621280'),bounded(D/'legacy-inputs.json',4262,'20a27d590c5055421ed049218361d6c3f91797e6654401076e7e1972dc842946'))
 previous=bounded(D/'before/tests__workspace__semantic_graph__source_inventory.rs.txt',*PRIOR).decode()
 def acquire(path,current):
  if path!=CARRIER:return current
  raw=current.encode()
  if (len(raw),hashlib.sha256(raw).hexdigest())==PRIOR:return current
  assert (len(raw),hashlib.sha256(raw).hexdigest())==CURRENT,('foreign complete repaired carrier',path)
  assert current.startswith(inverse['prefix'])
  restored=current[len(inverse['prefix']):]
  for row,edit in zip(legacy['rows'],inverse['edits']):
   # Authenticate each entire archived logical source at actual acquisition.
   bounded(R/row['archive'],row['bytes'],row['sha256'])
   assert edit['before']=='include_str!("../../'+row['logical']+'")'
   assert edit['after']=='semantic_graph_legacy_main_source!("'+row['logical']+'")'
  for edit in reversed(inverse['edits']):
   assert restored.count(edit['after'])==1
   restored=restored.replace(edit['after'],edit['before'],1)
  assert restored==previous
  return restored
 ns={'__file__':str(R/OLD)};exec(compile(old,str(R/OLD),'exec'),ns)
 m=ns['accepted_guard_extensions'](R)
 verify=m['verify_guard'];fragments=m['fragments']
 def verify_guard(path,current,original,refresh=False):
  assert not refresh,'frozen prior bindings cannot be retargeted'
  return verify(path,acquire(path,current),original,False)
 def finite_fragments(path,current,original):return fragments(path,acquire(path,current),original)
 m['verify_guard']=verify_guard;m['fragments']=finite_fragments
 return m
