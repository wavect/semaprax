"""Exact physical/whole-input import generation; no binder execution."""
from pathlib import Path
import hashlib,json,os,stat
REL=Path('docs/evidence/canonical-import-acquisition-7827-20261017')
TABLE_SHA='c0ec361ae90245676b4804403a4fe7bf65a34df06347109b086dfd0762deb7e0'
ROOT='src/cleanup_plan/replay/factored/canonical_shared.rs'
KEYS=(ROOT,ROOT[:-3]+'/arena_tests.rs')
sha=lambda b:hashlib.sha256(b).hexdigest()
def held(path,length,digest,limit=131072):
    assert hasattr(os,'O_NOFOLLOW') and 0<=length<=limit
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW),'rb')as f:
        before=os.fstat(f.fileno());assert stat.S_ISREG(before.st_mode) and before.st_size==length
        raw=f.read(length+1);assert len(raw)==length
        after=os.fstat(f.fileno());physical=os.lstat(path)
        identity=lambda x:(x.st_dev,x.st_ino,x.st_size,x.st_mode,x.st_mtime_ns,x.st_ctime_ns)
        assert stat.S_ISREG(physical.st_mode) and identity(before)==identity(after)==identity(physical)
    assert sha(raw)==digest;raw.decode('utf8');return raw
def acquire(repo,source,raw):
    if source not in KEYS:return raw
    repo=Path(repo);p=repo/REL/'sources.json'
    rows=json.loads(held(p,TABLE_LENGTH,TABLE_SHA,65536));assert tuple(r['source']for r in rows)==KEYS
    row=next(r for r in rows if r['source']==source)
    read=lambda name,kind:held(repo/name,row[kind+'_length'],row[kind+'_sha256'])
    actual=read(source,'current');assert actual==read(row['current_archive'],'current')
    stage=read(row['stage_archive'],'stage');restored=actual.decode()
    for e in row['inverse']:
        assert restored.count(e['after'])==1;restored=restored.replace(e['after'],e['before'],1)
    assert restored.encode()==stage
    assert raw in (actual,stage),'foreign canonical import input'
    return stage

TABLE_LENGTH=1807
