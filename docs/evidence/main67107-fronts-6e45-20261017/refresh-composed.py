#!/usr/bin/env python3
"""Exact common-refresh invocation adapter; original complete program stays unchanged."""
import hashlib,json,importlib.util
from pathlib import Path
sha=lambda b:hashlib.sha256(b).hexdigest()
def alias_stage(repo,source,raw):
    if source!='src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs':return raw
    helper=repo/'docs/evidence/main67107-fronts-6e45-20261017/composed_stage.py'
    assert sha(helper.read_bytes())=='d86e9260ffbb3328b7640d8907d45d62eebd7b5d978b0d5e683bd4efff472322'
    spec=importlib.util.spec_from_file_location('closed_outer_stage',helper);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    raw=module.outer_stage(repo,'ownedGrowth246fd230',source,raw)
    data=(repo/'docs/evidence/fade-custody-size-ownedgrowth-20261017/alias-root.json').read_bytes()
    assert sha(data)=='1e833dffa1cc56ba21a9c077b60be487ea41b172bc2e7312d954a6f424190fdf'
    rows=json.loads(data);assert len(rows)==3
    matching=[r for r in rows if r['source']==source];assert len(matching)==1
    row=matching[0];assert sha(raw)==row['current_sha256']
    text=raw.decode()
    for edit in row['edits']:
        assert text.count(edit['new'])==1
        text=text.replace(edit['new'],edit['old'],1)
    result=text.encode();assert sha(result)==row['input_sha256']=='a75c86aeb4a3e36f43e6ab963f6ff5357b8f9694c34250df32627a6ac513975b'
    return result
if __name__=='__main__':
    repo=Path(__file__).resolve().parents[3]
    original=repo/'docs/evidence/joined-batch-custody-20261016/refresh.py'
    old=original.read_bytes();assert sha(old)=='c2ab4323780232b8db4c18c66a864ad56409ca7b7329002833639fa27ce9f94d'
    needle="  before=old(d['commit'],f);raw=(R/f).read_bytes();archive=D/'before'/d['name']/f\n"
    replacement="  before=old(d['commit'],f);raw=(R/f).read_bytes()\n  if d['name']=='Alias1ae':raw=alias_stage(R,f,raw)\n  archive=D/'before'/d['name']/f\n"
    text=old.decode();assert text.count(needle)==1
    current=text.replace(needle,replacement,1)
    assert current.replace(replacement,needle,1).encode()==old
    exec(compile(current,str(original),'exec'),{'__file__':str(original),'__name__':'__main__','alias_stage':alias_stage})
