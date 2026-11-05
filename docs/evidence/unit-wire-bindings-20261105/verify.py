#!/usr/bin/env python3
"""Check complete fixture bytes, literal tuples, and every Operations binding."""
from pathlib import Path
import hashlib
import json
import re
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
packet = json.loads((HERE / 'manifest.json').read_bytes())
sha = lambda b: 'sha256:' + hashlib.sha256(b).hexdigest()
rows = {}
for row in packet['wires']:
    key = row['family'], row['host'], row['kind']
    assert key not in rows
    path = HERE / row['file']
    assert not path.is_symlink()
    body = path.read_bytes()
    assert len(body) == row['bytes'] and sha(body) == row['raw_sha256']
    assert body.endswith(b'\n')
    value = json.loads(body)
    if row['domain_digest']:
        assert row['domain_digest'] == sha((value['schema'][:-3] + '.artifact-digest.v1\0').encode() + body)
    rows[key] = body, value, row
assert len(rows) == 22
for pin in packet['pins']:
    for host in ['macos', 'portable']:
        row = rows[pin['family'], host, pin['kind']][2]
        assert pin[host] == (row['domain_digest'] or row['raw_sha256'])
code = (ROOT / 'src/source_reflows/unit_wire_pins.rs').read_text()
actual = re.findall(r'"(sha256:[0-9a-f]{64})"\s*=>\s*\(\s*"(sha256:[0-9a-f]{64})",\s*"(sha256:[0-9a-f]{64})",?\s*\)', code)
assert set(actual) == {(p['previous_literal'], p['macos'], p['portable']) for p in packet['pins']}
assert len(actual) == len(packet['pins']) == 10
for host in ['macos', 'portable']:
    def wire(kind):
        return rows['operations', host, kind]
    def artifact(kind):
        body, value, _ = wire(kind)
        suffix = '.proposal-digest.v1\0' if kind == 'change' else '.artifact-digest.v1\0'
        return sha((value['schema'][:-3] + suffix).encode() + body)
    child = wire('child-evidence')[0]
    assert wire('evidence')[1]['workspace_change_evidence']['document'] == child.decode()
    for kind in ['evidence', 'verification', 'application']:
        value = wire(kind)[1]
        for parent, field in [('derivation','operations_derivation'),('change','derived_workspace_change_proposal'),('child-evidence','workspace_change_evidence')]:
            assert value[field]['digest'] == artifact(parent)
            assert value[field]['bytes'] == len(wire(parent)[0])
        if kind != 'evidence':
            assert value['workspace_operations_evidence']['digest'] == artifact('evidence')
            assert value['workspace_operations_evidence']['bytes'] == len(wire('evidence')[0])
    assert wire('verification')[1]['result'] == 'exact_replay'
    assert wire('application')[1]['result'] == 'applied'

def compare(a, b, path=''):
    assert type(a) is type(b), path
    if isinstance(a, dict):
        assert a.keys() == b.keys(), path
        for key in a: compare(a[key],b[key],path+'/'+key)
    elif isinstance(a,list):
        assert len(a) == len(b), path
        for i,(x,y) in enumerate(zip(a,b)): compare(x,y,path+'/'+str(i))
    elif a != b:
        if path.endswith('/document'): compare(json.loads(a),json.loads(b),path)
        elif path.endswith('/digest'): assert a.startswith('sha256:') and b.startswith('sha256:'), path
        else:
            deltas={'used_candidate_graph_builder_bytes':144,'used_operations_builder_bytes':576,'used_analysis_builder_bytes':288}
            assert '/budget/' in path and path.rsplit('/',1)[1] in deltas, (path,a,b)
            assert a-b == deltas[path.rsplit('/',1)[1]], (path,a,b)
for family in ['operations','change']:
    kinds={key[2] for key in rows if key[0]==family}
    for kind in kinds: compare(rows[family,'macos',kind][1], rows[family,'portable',kind][1])
print('PASS: 22 complete fixture bodies, 10 literal tuples, all Operations bindings and declared builder deltas')
