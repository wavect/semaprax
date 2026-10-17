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
exec(compile(_raw,_m.__file__,'exec'),_m.__dict__)
_canonical_import_input=_m.acquire
_original=_held(N/'bind-selected-endpoints.before.py',1951,'05ad6ee46f9147227d0fece012c7ad118ec43c3e50aaa535d78025e4d46d4b49')
_edits=[{'before': 'def selected_previous(repo,raw):\n', 'after': "def selected_previous(repo,raw):\n    raw=_canonical_import_input(repo,'src/cleanup_plan/replay/factored/canonical_shared.rs',raw)\n"}]
_text=_original.decode()
for _e in _edits:assert _text.count(_e['before'])==1;_text=_text.replace(_e['before'],_e['after'],1)
_restored=_text
for _e in reversed(_edits):assert _restored.count(_e['after'])==1;_restored=_restored.replace(_e['after'],_e['before'],1)
assert _restored.encode()==_original
exec(compile(_text,str(P),'exec'),globals())
