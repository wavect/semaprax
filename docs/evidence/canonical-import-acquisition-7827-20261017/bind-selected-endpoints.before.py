"""Explicit later endpoint bind, never a capture/build/pin tool.
Authenticate actual new canonical bytes, recover the complete immediate f6ae
stage, then execute the whole previous singleton binder with one finite hook.
The parent must bind other integrated source endpoints from actual final HEAD.
"""
from pathlib import Path
import hashlib,json
D=Path(__file__).resolve().parent
R=D.parents[2]
sha=lambda raw:hashlib.sha256(raw).hexdigest()
PACKET_SHA='947c41698347ca27788aae5985e63f19c85d3f7eb3f25fee4374dac9d0d743d1'
BINDER_SHA='fb830593f4ccc544d8d54774a68ad5f823f9c96d2aa42dc75051cd6a74ec3978'
def selected_previous(repo,raw):
    packet=(D/'full-source-inverses.json').read_bytes();assert sha(packet)==PACKET_SHA
    row=json.loads(packet)[0]
    assert row['path']=='src/cleanup_plan/replay/factored/canonical_shared.rs'
    assert len(raw)==row['current_length'] and sha(raw)==row['current_sha256']
    text=raw.decode()
    for edit in row['inverse']:
        assert text.count(edit['after'])==1
        text=text.replace(edit['after'],edit['before'],1)
    before=(repo/row['archive']).read_bytes()
    assert len(before)==row['before_length'] and sha(before)==row['before_sha256']
    assert text.encode()==before
    return before

def render():
    original=(D/'bind-singleton-endpoints.before.py.txt').read_bytes()
    assert sha(original)==BINDER_SHA
    old=" assert sha(raw)==row['current_sha256']"
    new=" raw=_selected_previous(repo,raw)\n"+old
    source=original.decode();assert source.count(old)==1
    current=source.replace(old,new,1)
    assert current.replace(new,old,1).encode()==original
    return current

def main():
    path=R/'docs/evidence/ordinary-singleton-successors-20261017/bind-singleton-endpoints.py'
    assert sha(path.read_bytes())==BINDER_SHA
    exec(compile(render(),str(path),'exec'),{'__name__':'__main__','__file__':str(path),'_selected_previous':selected_previous})
if __name__=='__main__':main()
