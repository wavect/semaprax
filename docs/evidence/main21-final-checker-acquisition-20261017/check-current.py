"""Finite read-only acquisition before the complete immutable MAIN21 checker.
No Path monkeypatch, old table rewrite, binder, compiler or runtime execution.
"""
from pathlib import Path,PurePosixPath
import argparse,ast,hashlib,json,os,re,stat,subprocess
D=Path(__file__).resolve().parent
ORIGINAL='docs/evidence/main21-exit-region-integration-20261017/check-source.py'
ORIGINAL_LENGTH=6167
ORIGINAL_SHA='6191503c3d362261055ae169a0ffe08786f6b6feaf3cd8cdac310b1fca8bea24'
OLD_MANIFEST='docs/evidence/main21-exit-region-integration-20261017/input-domains.json'
OLD_MANIFEST_LENGTH=77210
OLD_MANIFEST_SHA='44687cd637b9ea867c84f6afccea40259a24b48231da21a2e76ed5ac4e9e740b'
OLD_INVERSES='docs/evidence/main21-exit-region-integration-20261017/full-source-inverses.json'
OLD_INVERSE_LENGTH=546539
OLD_INVERSE_SHA='71a32487c25ad83f9f65847593c668ddcefd17ebfeee598dd3b3e68991c1a9e2'
NEW_MANIFEST='docs/evidence/main09013-source-namespaces-v1/input-domains.json'
NEW_MODULE='src/source_reflows/incoming090_semantic_receivers.rs'
NEW_PROJECTION_SHA='1397d1267d6656fdd3bc1369f652ff5209ac468b21d615a887cc580e5dd95bd7'
OLD_CAP=9_382_389
NEW_CAP=9_464_242
sha=lambda raw:hashlib.sha256(raw).hexdigest()
def load(raw):
 def unique(pairs):
  out={}
  for key,value in pairs:assert key not in out;out[key]=value
  return out
 return json.loads(raw,object_pairs_hook=unique)
def physical(path,length=None,digest=None,cap=NEW_CAP):
 assert length is None or 0<=length<=cap
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 try:
  opened=os.fstat(fd);assert stat.S_ISREG(opened.st_mode)and opened.st_size<=cap
  if length is not None:assert opened.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as f:raw=f.read(opened.st_size);assert len(raw)==opened.st_size and not f.read(1)
  last=os.fstat(fd);named=path.lstat();identity=lambda s:(s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_ctime_ns,s.st_mode)
  assert stat.S_ISREG(named.st_mode)and identity(opened)==identity(last)==identity(named)
  raw.decode('utf8')
  if digest is not None:assert sha(raw)==digest
  return raw,identity(opened)
 finally:os.close(fd)
def projection(m):
 m=load(json.dumps(m).encode());m.pop('bound_final_head',None);m.pop('unassembled',None)
 for domain in ['main','physical']:
  for row in m[domain]:
   for key in ['current_len','current_sha256','binding_status']:row.pop(key,None)
 return (json.dumps(m,sort_keys=True,separators=(',',':'))+'\n').encode()
def render(raw):
 assert len(raw)==ORIGINAL_LENGTH and sha(raw)==ORIGINAL_SHA
 text=raw.decode();edits=[
  (" raw=(R/row['path']).read_bytes();assert sha(raw)==row['after_sha256']"," raw=_main21_inverse_raw(row);assert sha(raw)==row['after_sha256']"),
  ("  raw=(R/path).read_bytes();view=(R/row['archive']).read_bytes()","  raw=_main21_row_raw(row,path);view=(R/row['archive']).read_bytes()"),
  ("assert p.read_bytes()==(R/rel).read_bytes()==git(O,rel)","assert p.read_bytes()==_main21_legacy_bytes(rel)==git(O,rel)"),
  ("p='src/source_reflows/incoming67107_semantic_receivers.rs';value=(R/p).read_text()","p='src/source_reflows/incoming67107_semantic_receivers.rs';value=_main21_legacy_bytes(p).decode()"),
 ]
 for before,after in edits:assert text.count(before)==1;text=text.replace(before,after,1)
 restored=text
 for before,after in reversed(edits):assert restored.count(after)==1;restored=restored.replace(after,before,1)
 assert restored.encode()==raw;ast.parse(text)
 # The full original subsequent raw-three census has no acquisition hook.
 raw_start=raw.decode().index('# Unchanged strict token parser restores each COMPLETE a82 carrier')
 assert text[text.index('# Unchanged strict token parser restores each COMPLETE a82 carrier'):]==raw.decode()[raw_start:]
 return text,edits
def select_checkpoint(row,expected_length,expected_sha,raw,before):
 assert row['view_len']==expected_length<=OLD_CAP and row['view_sha256']==expected_sha
 assert len(before)==expected_length and sha(before)==expected_sha
 # Actual raw has already authenticated against the new physical manifest;
 # source selection can return only the complete explicitly named old stage.
 return before
class Acquisition:
 def __init__(self,root,expected_head):
  self.root=root;self.head=expected_head;self.observed={}
  self.git=lambda *args:subprocess.check_output(['git',*args],cwd=root)
  assert re.fullmatch('[0-9a-f]{40}',expected_head)
  assert self.git('rev-parse','HEAD').decode().strip()==expected_head
  original,_=physical(root/ORIGINAL,ORIGINAL_LENGTH,ORIGINAL_SHA)
  held,_=physical(D/'check-source.before.py.txt',ORIGINAL_LENGTH,ORIGINAL_SHA);assert held==original
  old,_=physical(root/OLD_MANIFEST,OLD_MANIFEST_LENGTH,OLD_MANIFEST_SHA,OLD_CAP)
  self.old=load(old);assert len(self.old['main'])==148 and len(self.old['ours'])==9
  inverse,_=physical(root/OLD_INVERSES,OLD_INVERSE_LENGTH,OLD_INVERSE_SHA,OLD_CAP)
  self.inverses={x['path']:x for x in load(inverse)};assert len(self.inverses)==11
  code,_=physical(root/NEW_MODULE,cap=1_048_576);code=code.decode()
  lens=re.findall(r'const MAIN090_MANIFEST_LEN: usize = ([0-9]+);',code)
  digests=re.findall(r'const MAIN090_MANIFEST_SHA: &str = "([0-9a-f]{64})";',code)
  assert len(lens)==len(digests)==1
  metadata,_=physical(root/NEW_MANIFEST,int(lens[0]),digests[0],1_048_576);self.new=load(metadata)
  assert sha(projection(self.new))==NEW_PROJECTION_SHA,'immutable MAIN090 namespaces drifted'
  assert len(self.new['main'])==173 and len(self.new['physical'])==174 and len(self.new['legacy_current'])==149 and len(self.new['expected'])==3
  self.current={x['path']:x for x in self.new['physical']};assert len(self.current)==174
  self.legacy={x['path']:x for x in self.new['legacy_current']};assert len(self.legacy)==149
  self.old_targets={}
  for domain in ['main','ours']:
   for row in self.old[domain]:
    pair=(row['current_len'],row['current_sha256']);path=row['path']
    assert path not in self.old_targets or self.old_targets[path]==pair
    self.old_targets[path]=pair
  assert set(self.old_targets)==set(self.legacy)
  for path,(length,digest)in self.old_targets.items():
   row=self.legacy[path];assert row['view_len']==length<=OLD_CAP and row['view_sha256']==digest
  paths=list(self.current)+[NEW_MODULE]
  for path in paths:assert not PurePosixPath(path).is_absolute()and '..'not in PurePosixPath(path).parts
  tree=self.git('ls-tree','-r','--full-tree',expected_head,'--',*paths).decode().splitlines();self.blobs={}
  for line in tree:
   header,path=line.split('\t',1);mode,kind,digest=header.split();assert kind=='blob'and mode in ['100644','100755'];self.blobs[path]=digest
  assert set(self.blobs)==set(paths)
  self.observe(NEW_MODULE,physical(root/NEW_MODULE,cap=1_048_576))
  for path,row in self.current.items():
   self.observe(path,physical(root/path,row['current_len'],row['current_sha256']))
  self.original=original
 def observe(self,path,value):
  raw,identity=value
  blob=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
  assert blob==self.blobs[path],('physical source differs from explicit Git HEAD',path)
  observed=(sha(raw),identity)
  if path in self.observed:assert self.observed[path]==observed,('physical source raced',path)
  self.observed[path]=observed
 def legacy_bytes(self,path):
  assert path in self.old_targets and path in self.current,'unknown finite MAIN21 checker acquisition'
  length,digest=self.old_targets[path];row=self.legacy[path];current=self.current[path]
  raw,identity=physical(self.root/path,current['current_len'],current['current_sha256']);self.observe(path,(raw,identity))
  before,_=physical(self.root/row['archive'],length,digest,OLD_CAP)
  return select_checkpoint(row,length,digest,raw,before)
 def row_raw(self,row,path):
  assert path==row['path']and self.old_targets[path]==(row['current_len'],row['current_sha256'])
  return self.legacy_bytes(path)
 def inverse_raw(self,row):
  assert row==self.inverses[row['path']]
  assert row['after_sha256']==self.old_targets[row['path']][1]
  return self.legacy_bytes(row['path'])
 def finish(self):
  assert self.git('rev-parse','HEAD').decode().strip()==self.head
  for path,(digest,identity)in self.observed.items():
   raw,after=physical(self.root/path,digest=digest);assert identity==after,('physical source raced',path)
  physical(self.root/OLD_MANIFEST,OLD_MANIFEST_LENGTH,OLD_MANIFEST_SHA,OLD_CAP)
  physical(self.root/OLD_INVERSES,OLD_INVERSE_LENGTH,OLD_INVERSE_SHA,OLD_CAP)
  assert self.git('rev-parse','HEAD').decode().strip()==self.head
def main():
 p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=D.parents[2]);p.add_argument('--expected-head',required=True);a=p.parse_args();root=a.repo.resolve()
 assert root==D.parents[2]and root.is_relative_to('/Volumes/Semaprax')
 assert os.environ.get('TMPDIR')=='/Volumes/Semaprax/runtime-tmp'and os.environ.get('PYTHONDONTWRITEBYTECODE')=='1'
 acquisition=Acquisition(root,a.expected_head);program,edits=render(acquisition.original)
 exec(compile(program,str(root/ORIGINAL),'exec'),{'__file__':str(root/ORIGINAL),'__name__':'__main__','_main21_inverse_raw':acquisition.inverse_raw,'_main21_row_raw':acquisition.row_raw,'_main21_legacy_bytes':acquisition.legacy_bytes})
 acquisition.finish()
 print(json.dumps({'status':'PASS_SOURCE_ONLY_FULL_MAIN21_CHECKER_WITH_FINITE_ACQUISITION','head':a.expected_head,'exact_acquisition_sites':len(edits),'physical_current_inputs':174,'complete_old_current_checkpoints':149,'old_manifest_unchanged':True,'subsequent_raw3_projected':False,'Cargo_or_Rust_or_binder_executed':False}))
if __name__=='__main__':main()
