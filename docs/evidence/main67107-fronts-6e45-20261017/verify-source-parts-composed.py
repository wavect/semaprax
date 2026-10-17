"""Exact Alias1ae source acquisition before unchanged whole source proofs.

The old parts helper still reconstructs only its fixed metadata arrays. The
single verifier source read projects ONLY Alias1ae + the owned-growth subject;
new-owner/raw reads, all other domains and all original assertions stay live.
"""
from pathlib import Path
import hashlib, importlib.util, sys
E=Path(__file__).resolve().parent
TARGET='src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs'
PARTS_SHA='e5563fee42e8743294947420298f7258282b5447bb294e42a8366efcaa703983'
VERIFIER_SHA='a90027caccbfbf183f08cdc6d4bd25ea5580403e0d1525099aa6342e020160c5'
COMPOSED_SHA='dbfb6f9dfbec702ed3f626588ceff61b9d454d08b5bd10f17dc1b07283cc3b64'
ACQUIRE_OLD="  raw=(R/r['source']).read_bytes();assert h(raw)==r['current_sha256'],r['source'];s=raw.decode()\n"
ACQUIRE_NEW="  raw=(R/r['source']).read_bytes()\n  if domain['name']=='Alias1ae' and r['source']==TARGET:raw=alias_stage(R,r['source'],raw)\n  assert h(raw)==r['current_sha256'],r['source'];s=raw.decode()\n"
PARTS_OLD="    runpy.run_path(str(original),run_name='__main__')\n"
PARTS_NEW="    exec(compile(projected_verifier,str(original),'exec'),{'__file__':str(original),'__name__':'__main__','alias_stage':alias_stage,'TARGET':TARGET})\n"
def programs(repo):
    repo=Path(repo).resolve()
    parts=repo/'docs/evidence/joined-batch-custody-20261016/verify-source-parts.py'
    original=repo/'docs/evidence/a3-e47-final-custody-20261016/verify-source.py'
    parts_raw=parts.read_bytes();verifier_raw=original.read_bytes()
    assert hashlib.sha256(parts_raw).hexdigest()==PARTS_SHA
    assert hashlib.sha256(verifier_raw).hexdigest()==VERIFIER_SHA
    helper=E/'refresh-composed.py';helper_raw=helper.read_bytes()
    assert hashlib.sha256(helper_raw).hexdigest()==COMPOSED_SHA
    spec=importlib.util.spec_from_file_location('exact_refresh_composition',helper)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    verifier=verifier_raw.decode();assert verifier.count(ACQUIRE_OLD)==1
    projected=verifier.replace(ACQUIRE_OLD,ACQUIRE_NEW,1)
    assert projected.replace(ACQUIRE_NEW,ACQUIRE_OLD,1).encode()==verifier_raw
    text=parts_raw.decode();assert text.count(PARTS_OLD)==1
    current=text.replace(PARTS_OLD,PARTS_NEW,1)
    assert current.replace(PARTS_NEW,PARTS_OLD,1).encode()==parts_raw
    return parts, current, projected, module.alias_stage
if __name__=='__main__':
    repo=Path(sys.argv[sys.argv.index('--repo')+1]).resolve()
    path, current, projected, alias_stage=programs(repo)
    exec(compile(current,str(path),'exec'),{'__file__':str(path),'__name__':'__main__','projected_verifier':projected,'alias_stage':alias_stage,'TARGET':TARGET})
