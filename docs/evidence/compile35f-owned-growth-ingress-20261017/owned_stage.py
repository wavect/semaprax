"""One exact source successor before the frozen seven-source compile channel."""
from pathlib import Path
import hashlib,json,os,stat
REL=Path('docs/evidence/compile35f-owned-growth-ingress-20261017')
TARGET='src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs'
SEAL_SHA='1481bc58a11f7e39d53cea0790414c98a6be1ccaa6509e990ac1e64084d095c8'
TABLE='docs/evidence/compile-acquisition-35f-20261017/sources.json'
sha=lambda raw:hashlib.sha256(raw).hexdigest()
def held(path,limit,expected=None):
    assert hasattr(os,'O_NOFOLLOW')
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW),'rb') as stream:
        before=os.fstat(stream.fileno());assert stat.S_ISREG(before.st_mode) and before.st_size<=limit
        raw=stream.read(before.st_size+1);assert len(raw)==before.st_size
        after=os.fstat(stream.fileno());physical=os.lstat(path)
        identity=lambda s:(s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_ctime_ns,s.st_mode)
        assert stat.S_ISREG(physical.st_mode) and identity(before)==identity(after)==identity(physical)
    if expected is not None:assert sha(raw)==expected
    raw.decode('utf8');return raw

def unique(pairs):
    result={}
    for key,value in pairs:assert key not in result;result[key]=value
    return result

def acquire(repo,read_fixed):
    repo=Path(repo);seal=json.loads(held(repo/REL/'seal.json',65536,SEAL_SHA),object_pairs_hook=unique)
    assert seal['schema']=='semaprax.compile35f-owned-growth-ingress.v1' and seal['source']==TARGET
    assert seal['original_table_length']<=1_048_576
    table=read_fixed(repo/TABLE,seal['original_table_length'],seal['original_table_sha256'])
    rows=json.loads(table,object_pairs_hook=unique);assert len(rows)==7
    matching=[row for row in rows if row['source']==TARGET];assert len(matching)==1;row=matching[0]
    assert row['current_sha256']==seal['previous_sha256'] and row['before_sha256']==seal['original35f_sha256']
    assert row['before_archive']==seal['original35f_archive']
    assert seal['current_length']<=262144 and seal['previous_length']<=262144 and seal['original35f_length']<=262144
    actual=read_fixed(repo/TARGET,seal['current_length'],seal['current_sha256'])
    assert actual==read_fixed(repo/seal['current_archive'],seal['current_length'],seal['current_sha256'])
    previous=read_fixed(repo/seal['previous_archive'],seal['previous_length'],seal['previous_sha256'])
    value=actual;last=len(value)
    for edit in seal['inverse']:
        start=edit['at'];after=edit['after'].encode();end=start+len(after)
        assert 0<=start<=end<=last and value[start:end]==after
        value=value[:start]+edit['before'].encode()+value[end:];last=start
    assert value==previous
    original=read_fixed(repo/seal['original35f_archive'],seal['original35f_length'],seal['original35f_sha256'])
    return actual,previous,original

def supplied(repo,source,raw,read_fixed):
    if source!=TARGET:return raw
    actual,previous,original=acquire(repo,read_fixed)
    if raw in (actual,previous):return previous
    assert raw==original,'foreign whole owned compile input'
    return original

def physical(repo,source,read_fixed,corrected_fixture):
    if source==TARGET:return acquire(repo,read_fixed)[1]
    # Preserve exactly the original corrected fixture physical path for other
    # sources; this function is reached only after the old sealed seven-key gate.
    return corrected_fixture(repo,source,(Path(repo)/source).read_bytes())

def stage_edits():
    return [
        (' raw=_corrected_fixture_stage(repo,source,raw)',
         ' raw=_owned_compile_input(repo,source,raw)\n raw=_corrected_fixture_stage(repo,source,raw)'),
        (' if source!=FACTORED_CORE:return _corrected_fixture_stage(repo,source,(repo/source).read_bytes())',
         ' if source!=FACTORED_CORE:return _owned_compile_physical(repo,source)'),
    ]
