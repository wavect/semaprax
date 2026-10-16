#!/usr/bin/env python3
"""Complete source composition only; no Rust/process/provider qualification."""
from pathlib import Path
import hashlib
import importlib.util
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
D = json.loads((HERE/"source-cuts.json").read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
def reject(operation):
    try:operation()
    except (AssertionError,ValueError):return
    raise AssertionError("drift accepted")
def previous(row,b):
    assert sha(b)==row["current_sha256"]
    for e in reversed(row["edits"]):
        a,z=e["start"],e["end"];assert b[a:z]==e["current"].encode()
        b=b[:a]+e["before"].encode()+b[z:]
    assert sha(b)==row["before_sha256"] and b==(ROOT/row["before"]).read_bytes()
    return b
def incoming(row,b):
    assert len(b)==row["current_length"] and sha(b)==row["current_sha256"]
    for e in reversed(row["edits"]):
        a,z=e["start"],e["end"];assert b[a:z]==e["current"].encode()
        b=b[:a]+e["incoming"].encode()+b[z:]
    assert len(b)==row["incoming_length"] and sha(b)==row["incoming_sha256"]
    assert b==(ROOT/row["incoming_archive"]).read_bytes()
    return b

def main():
    negatives=0
    for row in D["changed"]:
        b=(ROOT/row["path"]).read_bytes();previous(row,b)
        for wrong in [b"!"+b[1:],b+b" "]:
            reject(lambda:previous(row,wrong));negatives+=1
    for row in D["unchanged"]+D["complete_preimages"]:
        b=(ROOT/row["path"]).read_bytes();assert len(b)==row["bytes"] and sha(b)==row["sha256"],row["path"]
    rows=json.loads((HERE/"joined-inputs.json").read_text());assert len(rows)==3
    proof=json.loads((ROOT/"docs/evidence/factored-flat-targets-v1/sources.json").read_text())
    assert len(proof["predecessors"])==3 and not proof["new_files"]
    for row,old in zip(rows,proof["predecessors"]):
        assert row["path"]==old["path"]
        b=(ROOT/row["path"]).read_bytes();target=incoming(row,b).decode()
        assert sha(target.encode())==old["after_sha256"]
        restored=target
        for e in old["inverse"]:
            assert restored.count(e["after"])==1;restored=restored.replace(e["after"],e["before"],1)
        before=(ROOT/"docs/evidence/factored-flat-targets-v1"/old["archive"]).read_bytes()
        assert restored.encode()==before and sha(before)==old["before_sha256"]
        for wrong in [b"",b[:len(b)//2],b"!"+b[1:],b+b" "]:
            reject(lambda:incoming(row,wrong));negatives+=1
    root="src/cleanup_plan/replay/factored/canonical_shared.rs"
    tests="src/cleanup_plan/replay/factored/canonical_shared/tests.rs"
    oldroot=(HERE/"complete-preimages/ours"/root).read_text()
    assert (ROOT/root).read_text()==oldroot+'\n// Independent incoming receiver; paid Many production integration is OPT870.\n#[cfg(test)]\n#[path = "canonical_shared/incoming_flat_targets.rs"]\nmod targets;\n'
    oldtests=(HERE/"complete-preimages/ours"/tests).read_text()
    newblock='\nmod targets {'+(HERE/"complete-preimages/incoming"/tests).read_text().rsplit('\nmod targets {',1)[1]
    view='\n            let current = super::super::targets::incoming_input(record["path"].as_str().unwrap(), &current);'
    current=(ROOT/tests).read_text();assert current.startswith(oldtests)
    assert current[len(oldtests):].replace(view,'')==newblock
    incoming_root=(HERE/"complete-preimages/incoming"/root).read_text()
    originalbody=incoming_root.rsplit('\nmod targets {',1)[1][:-2]
    child=(ROOT/D["new_module"]).read_text()
    body=child.split('const JOINED_LENGTH:',1)[0].split('\n',1)[1].rstrip()
    assert body.replace('\n    type SharedWeighted = Rc<Weighted>;\n    use super::before_inline_weighted::{own_weighted, weighted_successor};','').rstrip()==originalbody.rstrip()
    oldproducer=(ROOT/"src/cleanup_plan/replay/factored/canonical_shared/before_inline_weighted.rs").read_text()
    for name in ["own_weighted","weighted_successor"]:
        pattern=r"fn "+name+r"\([\s\S]*?\n}\n"
        assert re.search(pattern,oldproducer).group()==re.search(pattern,incoming_root).group()
    assert (ROOT/"docs/evidence/unit-weight-closure-v1/inverse.json").read_bytes()==(HERE/"complete-preimages/ours/docs/evidence/unit-weight-closure-v1/inverse.json").read_bytes()
    data=(HERE/"joined-inputs.json").read_bytes()
    assert f"const JOINED_LENGTH: usize = {len(data)};" in child
    assert f'const JOINED_SHA256: &str = "{sha(data)}";' in child
    allowed_path="docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json"
    allowed=json.loads((ROOT/allowed_path).read_text());oldallowed=json.loads((HERE/"before"/allowed_path).read_text())
    for path,prefix in [("tests/hir_module_boundaries.rs","../"),("tests/workspace/semantic_graph/source_inventory.rs","../../"),("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs","../../../../../")]:
        raw=(ROOT/path).read_text();token='include_str!("'+prefix+D["new_module"]+'"),'
        assert raw.count(token)==1
        assert raw.replace(token,'',1)==(HERE/"before"/path).read_text()
        for key in ["required","maximum"]:assert allowed[path][key].pop(D["new_module"])==1
        row=next(row for row in D["changed"] if row["path"]==path)
        for wrong in [raw.replace(token,'',1),raw.replace(token,token+token,1),raw.replace(token,token.replace('incoming_flat_targets.rs','foreign.rs'),1)]:
            reject(lambda:previous(row,wrong.encode()));negatives+=1
    assert allowed==oldallowed
    assert len((ROOT/"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs").read_text().splitlines())==1500
    owners=json.loads((HERE/"selection-additions.json").read_text())["additions"];assert len(owners)==4
    for row in owners:assert newblock.count('fn '+row["selector"].split('::')[-1]+'(')==1 and row["status"]=='PENDING'
    # Original guard grammar exercised without endpoint writes.
    gp=ROOT/"docs/evidence/main043-independent-custody-20261016/guard-extensions.py"
    spec=importlib.util.spec_from_file_location("be8_guard",gp);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    for path in oldallowed:
        raw=(ROOT/path).read_text();old=subprocess.check_output(["git","show","a82e00cc8:"+path],cwd=ROOT).decode()
        for fragment in module.fragments(path,raw,old):raw=raw.replace(fragment,'',1)
        assert raw==old
    print(json.dumps(dict(status="PASS_SOURCE_ONLY",current_preimages=4,complete_merge_preimages=9,whole_stable_sources=10,original_source_inverses=3,raw_guards=3,negative_controls=negatives,new_pending_owners=4,production_body_unchanged=True,incoming_calls_assertions_unchanged=True,production_flat_many_adopted=False,rust_executed=False),sort_keys=True))

if __name__=="__main__":main()
