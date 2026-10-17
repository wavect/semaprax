"""One authenticated definition generation; no metadata or source writes."""
from pathlib import Path
import hashlib,json,os,re,stat
REL=Path('docs/evidence/main043-definition-generation-20261017')
TARGET='src/source_reflows/main043/semantic_receivers.rs'
SEAL_SHA='3c8b99fb381a110edfd712cf21dc60bccff81db875d3bac3d258eb37199100b8'
LIMIT=2*1024*1024
sha=lambda raw:hashlib.sha256(raw).hexdigest()
def bounded(path,length=None,digest=None,limit=LIMIT):
    assert hasattr(os,'O_NOFOLLOW')
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW),'rb') as stream:
        before=os.fstat(stream.fileno());assert stat.S_ISREG(before.st_mode)
        assert 0<=before.st_size<=limit
        if length is not None:assert before.st_size==length
        raw=stream.read(before.st_size+1);assert len(raw)==before.st_size
        after=os.fstat(stream.fileno())
        identity=lambda value:(value.st_dev,value.st_ino,value.st_size,value.st_mtime_ns,value.st_ctime_ns)
        assert identity(before)==identity(after)
        physical=os.lstat(path);assert stat.S_ISREG(physical.st_mode)
        assert identity(after)==identity(physical)
    if digest is not None:assert sha(raw)==digest
    raw.decode('utf8');return raw

def unique(pairs):
    result={}
    for key,value in pairs:assert key not in result;result[key]=value
    return result

def definition_payload(root,row,original_read):
    # Every other definition read uses the entire unchanged original algorithm.
    if row['path']!=TARGET:return original_read(row['path'])
    root=Path(root).resolve()
    seal_raw=bounded(root/REL/'definition-seal.json',digest=SEAL_SHA,limit=65536)
    seal=json.loads(seal_raw,object_pairs_hook=unique)
    assert seal['schema']=='semaprax.main043-definition-generation.v1'
    assert seal['path']==TARGET and row==seal['old_row']
    current=bounded(root/TARGET,seal['current_length'],seal['current_sha256'])
    assert current==bounded(root/seal['current_archive'],seal['current_length'],seal['current_sha256'])
    # These are actual raw source scopes, never the historical definition view.
    assert len(seal['raw_inventories'])==3
    expected={
        'tests/hir_module_boundaries.rs',
        'tests/workspace/semantic_graph/source_inventory.rs',
        'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs',
    }
    assert {item['path'] for item in seal['raw_inventories']}==expected
    for item in seal['raw_inventories']:
        raw=bounded(root/item['path'],item['length'],item['sha256'])
        names=re.findall(r'include_str!\(\s*"([^"\n]+)"',raw.decode())
        assert sum(name.endswith('/'+TARGET) for name in names)==item['exact_target_include_count']==1
    restored=current;last=len(restored)
    for edit in seal['inverse']:
        start=edit['at'];after=edit['after'].encode();end=start+len(after)
        assert 0<=start<=end<=last and restored[start:end]==after
        restored=restored[:start]+edit['before'].encode()+restored[end:];last=start
    old=bounded(root/seal['old_archive'],row['bytes'],row['sha256'])
    assert restored==old and len(restored)==row['bytes'] and sha(restored)==row['sha256']
    return restored.decode('utf8')
