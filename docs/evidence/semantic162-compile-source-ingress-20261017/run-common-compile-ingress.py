"""Exact compilation source ingress before the complete unchanged common scripts.
Only the one repaired source in the immutable seven-key channel is projected.
"""
from pathlib import Path
import hashlib,json,os,stat
P=Path(__file__).resolve();R=P.parents[3];D=P.parent
sha=lambda b:hashlib.sha256(b).hexdigest()
def held(path,length,expected):
    assert 0<=length<=65536
    flags=os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC
    fd=os.open(path,flags)
    try:
        first=os.fstat(fd);assert stat.S_ISREG(first.st_mode)and first.st_size==length
        with os.fdopen(fd,'rb',closefd=False)as stream:
            raw=stream.read(length+1);assert len(raw)==length
        ident=lambda x:(x.st_dev,x.st_ino,x.st_mode,x.st_size,x.st_mtime_ns,x.st_ctime_ns)
        assert ident(first)==ident(os.fstat(fd))==ident(path.lstat())and sha(raw)==expected
        raw.decode('utf8');return raw
    finally:os.close(fd)
ORIGINAL='docs/evidence/alias-fixtures-c874-20261017/run-common-corrected-fixtures.py'
raw=held(D/'run-common.before.py.txt',3098,'40e4ea049c5d86600ab59bf463f9c1cdae03c647c7bd46213e19e755a38df570')
assert raw==held(R/ORIGINAL,len(raw),sha(raw))
text=raw.decode();old_child=ORIGINAL;new_child='docs/evidence/semantic162-compile-source-ingress-20261017/run-common-compile-ingress.py'
assert text.count(old_child)==1
current=text.replace(old_child,new_child,1)
assert current.replace(new_child,old_child,1).encode()==raw
namespace={'__file__':str(R/ORIGINAL),'__name__':'held_original_common_frontend'}
exec(compile(current,str(R/ORIGINAL),'exec'),namespace)
old_supplied=namespace['_owned_supplied'];old_physical=namespace['_owned_physical']
TARGET='src/cleanup_plan/replay/sequence_prefix.rs'
def previous(repo,source,raw,read_fixed):
    if source!=TARGET:return raw
    data=held(Path(repo)/'docs/evidence/semantic162-compile-source-ingress-20261017/inputs.json',13993,'e3870d5c067c099197b21f5b749b3124eaa4174701fe3eb47ca73db88ac44f14')
    packet=json.loads(data);rows=packet['rows'];assert len(rows)==16
    selected=[r for r in rows if r['source']==TARGET];assert len(selected)==1;row=selected[0]
    actual=read_fixed(Path(repo)/source,row['current_length'],row['current_sha256'])
    assert actual==read_fixed(Path(repo)/row['current_archive'],row['current_length'],row['current_sha256'])
    before=read_fixed(Path(repo)/row['previous_archive'],row['previous_length'],row['previous_sha256'])
    restored=actual;last=len(actual)
    for edit in row['inverse']:
        start=edit['start'];after=edit['after'].encode();end=start+len(after)
        assert 0<=start<=end<=last and restored[start:end]==after
        restored=restored[:start]+edit['before'].encode()+restored[end:];last=start
    assert restored==before
    table=namespace['m'].held(Path(repo)/'docs/evidence/compile-acquisition-35f-20261017/sources.json',1048576,'61af497922931df374510a7d8acd4b8691481f960407f1670c4d4e4dc37625fe')
    old=next(r for r in json.loads(table)if r['source']==TARGET);assert old['current_sha256']==row['previous_sha256']
    return before if raw in(actual,before)else raw
namespace['_owned_supplied']=lambda repo,source,raw,read_fixed:old_supplied(repo,source,previous(repo,source,raw,read_fixed),read_fixed)
namespace['_owned_physical']=lambda repo,source,read_fixed,fixture:previous(repo,source,old_physical(repo,source,read_fixed,fixture),read_fixed)
# Original importable API remains available; original __main__ CLI runs once.
render_stage=namespace['render_stage']
if __name__=='__main__':namespace['main']()
