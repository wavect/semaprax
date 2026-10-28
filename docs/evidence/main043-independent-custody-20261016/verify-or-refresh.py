#!/usr/bin/env python3
"""Fixed one-definition generation ingress; full original verifier retained."""
from pathlib import Path
import hashlib,types,os,stat
P=Path(__file__).resolve();R=P.parents[3]
D=R/'docs/evidence/main043-definition-generation-20261017'
sha=lambda raw:hashlib.sha256(raw).hexdigest()
# Explicit fixed prior semantic-helper domain, never a wildcard source read.
SEMANTIC_HELPER_SHA='f1c6e5db4d0bdba759834c0b80ee76ef4cdf97d7743afa77395979d96589f9aa'
def held(path,length):
    assert hasattr(os,'O_NOFOLLOW')
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW),'rb') as stream:
        before=os.fstat(stream.fileno());assert stat.S_ISREG(before.st_mode)
        assert before.st_size==length and length<=65536
        raw=stream.read(length+1);assert len(raw)==length
        after=os.fstat(stream.fileno())
        identity=lambda value:(value.st_dev,value.st_ino,value.st_size,value.st_mtime_ns,value.st_ctime_ns)
        assert identity(before)==identity(after)
        physical=os.lstat(path);assert stat.S_ISREG(physical.st_mode)
        assert identity(after)==identity(physical)
    return raw
original=held(D/'verify-or-refresh.before.py',9518)
assert len(original)==9518 and sha(original)=='7c2eb0ca8ff31f89b3744d69089bb62c57c8114495ddc4ec78862c32f8b4392a'
assert original.decode().count(SEMANTIC_HELPER_SHA)==1
helper=R/'docs/evidence/next-big-batch-source-binding-20261017/definition_adapter.py';raw=held(helper,1766)
assert len(raw)==1766 and sha(raw)=='ab6e2d1c227dee30da804701dd3d5f54c57c853c08ce423f98359a8420cf2731'
m=types.ModuleType('fixed_main043_definition_payload')
m.__file__=str(helper)
exec(compile(raw,str(helper),'exec'),m.__dict__)
old=" payload=read(row['path'])"
new=" payload=_definition_payload(R,row,read)"
source=original.decode();assert source.count(old)==1
current=source.replace(old,new,1)
assert current.replace(new,old,1).encode()==original
guard_helper=R/'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';guard_raw=held(guard_helper,11549)
assert sha(guard_raw)=='c7f687b4552af9edf5350008d7620f1b73649b4e5eba8434c3bef153ed97ff83'
g=types.ModuleType('accepted_main043_strict_guard');g.__file__=str(guard_helper);g._AUTHENTICATED_PROGRAM=guard_raw
exec(compile(guard_raw,str(guard_helper),'exec'),g.__dict__)
m._next_raw_source=g.source
guard_old="runpy.run_path(str(R/D/'guard-extensions.py'))"
guard_new="_accepted_guard_extensions(R)"
assert current.count(guard_old)==2
current=current.replace(guard_old,guard_new)
assert current.replace(guard_new,guard_old).replace(new,old,1).encode()==original
exec(compile(current,str(P),'exec'),{'__name__':'__main__','__file__':str(P),'_definition_payload':m.definition_payload,'_accepted_guard_extensions':g.accepted_guard_extensions})
