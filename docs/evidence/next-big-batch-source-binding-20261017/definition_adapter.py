"""One named raw3 acquisition before the complete original definition reader."""
from pathlib import Path
import hashlib,os,stat
R=Path(__file__).resolve().parents[3]
def held(path,length,digest):
 assert 0<=length<=262144
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  before=os.fstat(fd);assert stat.S_ISREG(before.st_mode)and before.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as f:raw=f.read(length);assert len(raw)==length and not f.read(1)
  assert identity(before)==identity(os.fstat(fd))==identity(os.lstat(path));assert hashlib.sha256(raw).hexdigest()==digest;return raw
 finally:os.close(fd)
def definition_payload(root,row,original_read):
 assert Path(root).resolve()==R
 path='docs/evidence/main043-definition-accepted-generation-20261017/definition_payload.py'
 raw=held(R/path,3228,'59e731713eea180fec3a07c7eb818633591ed948cf4462c5796dcd1810ec36f3')
 assert raw==held(R/'docs/evidence/next-big-batch-source-binding-20261017/guards/definition.whole-before.py.txt',3228,'59e731713eea180fec3a07c7eb818633591ed948cf4462c5796dcd1810ec36f3')
 old="raw=bounded(root/item['path'],item['length'],item['sha256'])"
 new="raw=_next_raw_source('root',item['path'],item['length'],item['sha256'])"
 code=raw.decode();assert code.count(old)==1;current=code.replace(old,new,1);assert current.replace(new,old,1).encode()==raw
 # The public held bootstrap injects this exact callback only after holding
 # the new guard. No physical reader or ambient fallback is substituted.
 ns=dict(__file__=str(R/path),_next_raw_source=_next_raw_source)
 exec(compile(current,str(R/path),'exec'),ns)
 return ns['definition_payload'](R,row,original_read)
