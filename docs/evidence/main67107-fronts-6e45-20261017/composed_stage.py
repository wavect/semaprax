"""Exact current canonical/owned-growth acquisition before unchanged original stages."""
import hashlib,json,importlib.util
from pathlib import Path
DATA_SHA256 = "bfbacf2777c310ee34ad6acd34352a9c58777ec11d39f2649be584711d72f120"
def outer_stage(repo,domain,source,raw):
    allowed={"canonical870":"src/cleanup_plan/replay/factored/canonical_shared.rs","ownedGrowth246fd230":"src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs"}
    assert domain in allowed and source==allowed[domain]
    repo=Path(repo);data=(repo/"docs/evidence/main67107-fronts-6e45-20261017/stages.json").read_bytes();sha=lambda b:hashlib.sha256(b).hexdigest()
    assert len(data)<=1048576 and sha(data)==DATA_SHA256
    rows=json.loads(data);assert len(rows)==9
    matching=[r for r in rows if r["domain"]==domain];assert len(matching)==1
    row=matching[0];assert row["source"]==source
    assert sha(raw)==row["current_sha256"] and raw.decode()==row["current"]
    lines=raw.decode().splitlines(keepends=True);last=len(lines)
    assert 0<len(row["inverse"])<=512
    for e in row["inverse"][::-1]:
        a,z=e["start"],e["end"];assert 0<=a<=z<=last<=len(lines)
        assert "".join(lines[a:z])==e["current"]
        lines[a:z]=e["previous"].splitlines(keepends=True);last=a
    old="".join(lines).encode();assert len(old)==row["previous_length"] and sha(old)==row["previous_sha256"]
    return old
def canonical_stage(repo,raw):
    repo=Path(repo);sha=lambda b:hashlib.sha256(b).hexdigest()
    old=outer_stage(repo,"canonical870","src/cleanup_plan/replay/factored/canonical_shared.rs",raw)
    helper=repo/"docs/evidence/opt870-current-stage-custody-20261017/canonical870_stage.py"
    assert sha(helper.read_bytes())=="1e85e09ad2eb5ec2189ce3915f389726d1219b09c85be997b39ae0feed6a0061"
    spec=importlib.util.spec_from_file_location("frozen870stage",helper);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module.canonical_stage(repo,old)
