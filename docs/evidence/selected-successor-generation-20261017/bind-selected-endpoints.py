"""Fixed compile-import generation before the complete Selected binder."""
from pathlib import Path
import types
P=Path(__file__).resolve();R=P.parents[3];N=R/'docs/evidence/canonical-import-acquisition-7827-20261017'
# Bootstrap bounded held-byte reader, exact helper source before execution.
import os,stat,hashlib
def _held(path,length,digest):
    assert hasattr(os,'O_NOFOLLOW') and length<=65536
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW),'rb') as f:
        before=os.fstat(f.fileno());assert stat.S_ISREG(before.st_mode) and before.st_size==length
        raw=f.read(length+1);assert len(raw)==length
        after=os.fstat(f.fileno());physical=os.lstat(path)
        identity=lambda x:(x.st_dev,x.st_ino,x.st_size,x.st_mode,x.st_mtime_ns,x.st_ctime_ns)
        assert stat.S_ISREG(physical.st_mode) and identity(before)==identity(after)==identity(physical)
    assert hashlib.sha256(raw).hexdigest()==digest;return raw
_raw=_held(N/'acquire.py',1825,'0548bc742a3a2b25271717b23673657f17c2b815baea538fb57d20f086d5a10f')
_m=types.ModuleType('canonical_import_acquisition');_m.__file__=str(N/'acquire.py')

_paid_data=_held(R/'docs/evidence/paid-many-canonical-ingress-20261017/seal.json',2923,'890336922097f28cddddbcfe85a3c267bf148acd9419289b198eb081fc8060be')
_paid_seal=__import__('json').loads(_paid_data)
def _paid_previous(repo,source,raw=None):
    if source!=_paid_seal['source']:return raw
    actual=_m.held(Path(repo)/source,_paid_seal['current_length'],_paid_seal['current_sha256'])
    assert actual==_m.held(Path(repo)/_paid_seal['current_archive'],_paid_seal['current_length'],_paid_seal['current_sha256'])
    previous=_m.held(Path(repo)/_paid_seal['previous_archive'],_paid_seal['previous_length'],_paid_seal['previous_sha256'])
    restored=actual;last=len(restored)
    for edit in _paid_seal['inverse']:
        start=edit['start_byte'];after=edit['after'].encode();end=start+len(after)
        assert end<=last and restored[start:end]==after
        restored=restored[:start]+edit['before'].encode()+restored[end:];last=start
    assert restored==previous
    return previous if raw is None or raw in (actual,previous)else raw
_m.__dict__['_paid_physical']=lambda repo,source,row:_paid_previous(repo,source)if source==_paid_seal['source']else _m.held(Path(repo)/source,row['current_length'],row['current_sha256'])
_helper_text=_raw.decode()
_old_acquisition="actual=read(source,'current');assert actual==read(row['current_archive'],'current')"
_new_acquisition="actual=_paid_physical(repo,source,row);assert actual==read(row['current_archive'],'current')"
assert _helper_text.count(_old_acquisition)==1
_helper_text=_helper_text.replace(_old_acquisition,_new_acquisition,1)
assert _helper_text.replace(_new_acquisition,_old_acquisition,1).encode()==_raw
exec(compile(_helper_text,_m.__file__,'exec'),_m.__dict__)

_canonical_import_input=lambda repo,source,raw:_m.acquire(repo,source,_paid_previous(repo,source,raw))
_original=_held(N/'bind-selected-endpoints.before.py',1951,'05ad6ee46f9147227d0fece012c7ad118ec43c3e50aaa535d78025e4d46d4b49')
_edits=[{'before': 'def selected_previous(repo,raw):\n', 'after': "def selected_previous(repo,raw):\n    raw=_canonical_import_input(repo,'src/cleanup_plan/replay/factored/canonical_shared.rs',raw)\n"}]
_text=_original.decode()
for _e in _edits:assert _text.count(_e['before'])==1;_text=_text.replace(_e['before'],_e['after'],1)
_restored=_text
for _e in reversed(_edits):assert _restored.count(_e['after'])==1;_restored=_restored.replace(_e['after'],_e['before'],1)
assert _restored.encode()==_original
exec(compile(_text,str(P),'exec'),globals())
