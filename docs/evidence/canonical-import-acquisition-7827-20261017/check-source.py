"""Whole original canonical source controls, with finite PaidMany acquisition."""
from pathlib import Path
import hashlib,json,os,stat
P=Path(__file__).resolve();R=P.parents[3];N=R/'docs/evidence/paid-many-canonical-ingress-20261017'
def _held(path,length,digest):
    assert hasattr(os,'O_NOFOLLOW') and 0<=length<=65536
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC),'rb') as f:
        before=os.fstat(f.fileno())
        assert stat.S_ISREG(before.st_mode) and before.st_size==length
        raw=f.read(length)
        assert len(raw)==length and f.read(1)==b''
        after=os.fstat(f.fileno());physical=os.lstat(path)
        identity=lambda x:(x.st_dev,x.st_ino,x.st_mode,x.st_size,x.st_mtime_ns,x.st_ctime_ns)
        assert stat.S_ISREG(physical.st_mode) and identity(before)==identity(after)==identity(physical)
    assert hashlib.sha256(raw).hexdigest()==digest
    return raw
before=_held(N/'check-source.before.py.txt',6063,'85055bcfa1ef269afa779835e5e6b399af772b283a7a29bbf5777c9f6fd80ae9')
edits=_held(N/'checker-edits.json',780,'c3afed2626ffcfc6fb214cfb64ff4bb50ddf5ced7d7cf3c6f467f44ca1d99c4a')
text=before.decode()
for old,new in json.loads(edits):assert text.count(old)==1;text=text.replace(old,new,1)
restored=text
for old,new in reversed(json.loads(edits)):assert restored.count(new)==1;restored=restored.replace(new,old,1)
assert restored.encode()==before
exec(compile(text,str(P),'exec'),{'__file__':str(P)})
