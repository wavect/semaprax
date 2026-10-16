#!/usr/bin/env python3
"""Finite source-only proof; does not execute Rust or certify Session closure."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();root=a.repo.resolve()
packet=root/'docs/evidence/lexical-type-drop-paid-receiving-781-next-20261016'
data=json.loads((packet/'source-inverses.json').read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
for row in data['rows']:
    old=(packet/row['before_archive']).read_bytes();current=(root/row['source']).read_bytes()
    assert sha(old)==row['before_sha256'];assert sha(current)==row['after_sha256']
    assert old==subprocess.check_output(['git','show',data['base']+':'+row['source']],cwd=root)
    forward=old.decode();reverse=current.decode()
    for e in reversed(row['edits']):
        i=e['old_offset'];assert forward[i:i+len(e['before'])]==e['before']
        forward=forward[:i]+e['after']+forward[i+len(e['before']):]
        i=e['new_offset'];assert reverse[i:i+len(e['after'])]==e['after']
        reverse=reverse[:i]+e['before']+reverse[i+len(e['after']):]
    assert forward.encode()==current;assert reverse.encode()==old
for row in data['new_sources']:
    source=root/row['source'];assert sha(source.read_bytes())==row['sha256']
    assert len(source.read_text().splitlines())==row['lines']<=1500

def original(path):return subprocess.check_output(['git','show',data['base']+':'+path],cwd=root)
for tail,new in [('paid.rs','before_type_drop.rs'),('paid/storage.rs','before_type_drop/storage.rs')]:
    path='src/cleanup_plan/replay/lexical_cert/'
    assert (root/(path+new)).read_bytes()==original(path+tail)
for row in json.loads((packet/'original-query-inputs.json').read_text()):
    old=(packet/row['archive']).read_bytes();assert sha(old)==row['sha256']
    assert old==original(row['source'])==(root/row['source']).read_bytes()
for prefix in ['structural-leaf','status-scalar','certificate-precision','boundary-chain']:
    prior='docs/evidence/'+prefix+'-paid-receiving-781-next-20261016/'
    for path in subprocess.check_output(['git','ls-tree','-r','--name-only',data['base'],prior],cwd=root).decode().splitlines():
        assert (root/path).read_bytes()==original(path)
caps={}
for line in (root/'tests/module-size-budget.tsv').read_text().splitlines():
    if not line or line.startswith('#'):continue
    cells=line.split('\t')
    if len(cells)>1 and cells[1].isdigit():caps[cells[0]]=int(cells[1])
for row in data['rows']:
    if row['source'].endswith('.rs'):
        n=len((root/row['source']).read_text().splitlines());assert n<=caps.get(row['source'],1500),(row['source'],n)
macro=(root/'src/cleanup_plan/replay/selected_type_drop.rs').read_text()
for name in ['primitive_drop','needs_drop_in','vector_element','authenticate_vector','flat_fields','flat_declaration','type_needs_drop']:
    assert 'macro_rules! '+name in macro
assert 'allocation::authority(program, function)' in macro
assert 'with_skeleton_limit_oracle' in macro and 'type_needs_resource_cleanup' in macro
assert 'reserve_' not in macro and 'Permit::' not in macro
assert 'else if' not in macro
assert 'selected_type_drop::type_needs_drop!' in (root/'src/cleanup_plan/replay/lexical_cert/paid.rs').read_text()
assert len(json.loads((packet/'pending-selectors.json').read_text()))==4
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_before_inverse_rows':len(data['rows']),'original_query_inputs':5,'new_sources':len(data['new_sources']),'independent_prior_certificate_storage':True,'rust_executed':False,'issue_781':'OPEN_RESIDUAL_DESCENDANTS'},sort_keys=True))
