"""One finite branch-test generation, preserving the complete original strict binder.
Execution requires an explicitly reviewed full HEAD. No compiler is invoked.
"""
from pathlib import Path
import hashlib,os,stat
D=Path(__file__).resolve().parent
REL='docs/evidence/main090-paid-many-next-binding-20261017/'
OLD='docs/evidence/main09013-source-namespaces-v1/bind_current.py'
BEFORE_LEN=241192
BEFORE_SHA='f0f48329a8b27b4b83474e94f6449f88e2e432ebbf4cf5b05d053acc9ba79f78'
BINDER_LEN=13491
BINDER_SHA='4921dedbcd93d865cafc825f176f5598891864f35c605096185dfb49c6887546'
CHANGED={'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs': (155342, '2f60338ca315635e7d533ba6cbdeadda40d8b2e3d66a7398c7521d4b44f5f845'), 'src/cleanup_plan/replay.rs': (321092, '30616d71694d393a730c0c8e49a25bbad7539b07ff96ed901b9d07be29765b9c'), 'src/source_reflows/incoming67107_semantic_receivers.rs': (32535, '70136f31e90d3055f8622183540e6bcaf83c7ac29409dd0ac51715c89c9c6ee3'), 'tests/hir_module_boundaries.rs': (97256, 'f25b784eba1a2ea26a07b260b2ddff67b6dd90c0a6402fd143dc42ca68847d4e'), 'tests/workspace/semantic_graph/source_inventory.rs': (90364, '59e5b98593c2c2d443d1479e7c89564551d515ca921f7abab2cc8f5d579c8c21')}

sha=lambda b:hashlib.sha256(b).hexdigest()
def held(path,length,digest):
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode)and first.st_size==length<=262144
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(length);assert len(b)==length and not f.read(1)
  assert identity(first)==identity(os.fstat(fd))==identity(path.lstat())and sha(b)==digest
  return b
 finally:os.close(fd)
def guard_physical(m,physical):
 prior={x['path']:(x['current_len'],x['current_sha256'])for x in m['physical']}
 assert len(prior)==174 and set(prior)==set(physical)
 assert set(CHANGED)<=set(prior)
 for path,pair in physical.items():
  assert pair==(CHANGED[path]if path in CHANGED else prior[path]),('foreign next-generation physical input',path)
def render(raw):
 assert len(raw)==BINDER_LEN and sha(raw)==BINDER_SHA
 text=raw.decode()
 edits=[
  ("before=current(d['manifest']);require(before==template,'one-shot binder requires exact original template')",
   "before=current(d['manifest']);require(len(before)==NEXT_BEFORE_LEN and sha(before)==NEXT_BEFORE_SHA,'finite next binder requires exact retained complete preceding generation')\n require(current(NEXT_REL+'input-domains.before.json')==before,'retained prior generation drift')\n require(current(NEXT_REL+'bind_current.before.py.txt')==current(d['binder']),'whole original binder drift')\n current(NEXT_REL+'bind-next.py')"),
  ("m=parse(template);require(m['table_head']", "m=parse(before);require(m['table_head']"),
  ("require(set(row['path'] for row in m['main'])<=set(physical),'missing physical domain')",
   "require(set(row['path'] for row in m['main'])<=set(physical),'missing physical domain')\n next_guard_physical(m,physical)"),
 ]
 for before,after in edits:assert text.count(before)==1;text=text.replace(before,after,1)
 restored=text
 for before,after in reversed(edits):assert restored.count(after)==1;restored=restored.replace(after,before,1)
 assert restored.encode()==raw
 return text

def main():
 raw=held(D/'bind_current.before.py.txt',BINDER_LEN,BINDER_SHA)
 held(D/'input-domains.before.json',BEFORE_LEN,BEFORE_SHA)
 # Complete original CLI, root FD reader, descriptor, namespace mask, old
 # template, raw3 parser, 174 Git acquisitions, race rechecks, O_EXCL archive,
 # staging and publication run from authenticated held code without reopen.
 exec(compile(render(raw),str(D/ 'bind_current.before.py.txt'),'exec'),{
  '__name__':'__main__','__file__':str(D/'bind_current.before.py.txt'),
  'NEXT_REL':REL,'NEXT_BEFORE_LEN':BEFORE_LEN,'NEXT_BEFORE_SHA':BEFORE_SHA,
  'next_guard_physical':guard_physical,
 })
if __name__=='__main__':main()
