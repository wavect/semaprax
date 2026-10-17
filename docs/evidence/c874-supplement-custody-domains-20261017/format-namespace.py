"""One exact RAW namespace source acquisition before unchanged BCC assertions."""
from pathlib import Path
import hashlib,json,os,stat
META_LENGTH=607
META_SHA='1c120e26c852966dba70f8d67b5869465bedd655aa4eb4612986453a2f8f1035'
ROOT_SOURCE='src/source_reflows/semantic_receivers.rs'
def read(path,length,sha):
 assert 0<=length<=65536
 flags=os.O_RDONLY|getattr(os,'O_NOFOLLOW',0)|getattr(os,'O_NONBLOCK',0)
 fd=os.open(path,flags)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode) and first.st_size==length
  with os.fdopen(fd,'rb',closefd=False) as f:
   raw=f.read(length);assert len(raw)==length and f.read(1)==b''
  last=os.fstat(fd);physical=os.lstat(path)
  assert not stat.S_ISLNK(physical.st_mode)
  identity=lambda m:(m.st_dev,m.st_ino,m.st_size,m.st_mtime_ns,m.st_ctime_ns)
  assert identity(first)==identity(last)==identity(physical)
  assert hashlib.sha256(raw).hexdigest()==sha
  return raw.decode('utf8')
 finally:os.close(fd)
def previous_namespace(repo,current):
 root=Path(repo);d=root/'docs/evidence/c874-supplement-custody-domains-20261017'
 row=json.loads(read(d/'receiver-input.json',META_LENGTH,META_SHA));assert row['source']==ROOT_SOURCE
 raw=read(root/ROOT_SOURCE,row['current_length'],row['current_sha256'])
 assert current.encode('utf8')==raw.encode('utf8'),'provided RAW namespace must match physical bytes'
 assert raw==read(d/row['current_archive'],row['current_length'],row['current_sha256'])
 assert row['format_sha256']=='ba2ddbf8486039ba24fb0f36a90dea66910e931cbb71115b4d2eab114906430b'
 return read(d/row['format_archive'],row['format_length'],row['format_sha256'])
