#!/usr/bin/env python3
"""Two complete input inverses and immutable 303d proof replay; source only."""
import argparse, hashlib, json, re, runpy, subprocess, sys
from pathlib import Path
p = argparse.ArgumentParser(); p.add_argument('--repo', type=Path, default=Path.cwd())
root = p.parse_args().repo.resolve()
packet = root / 'docs/evidence/static-initial-layout-781-next-20261016'
data = json.loads((packet / 'source-inverses.json').read_text())
sha = lambda b: hashlib.sha256(b).hexdigest()
projected = {}
for row in data['rows']:
    old = (packet / row['before_archive']).read_bytes()
    current = (root / row['source']).read_bytes()
    assert old == subprocess.check_output(['git', 'show', data['base'] + ':' + row['source']], cwd=root)
    assert sha(old) == row['before_sha256'] and sha(current) == row['after_sha256']
    a,b,i = old.decode(),current.decode(),row['offset']
    assert a[i:i+len(row['before'])] == row['before']
    assert b[i:i+len(row['after'])] == row['after']
    assert a[:i]+row['after']+a[i+len(row['before']):] == b
    assert b[:i]+row['before']+b[i+len(row['after']):] == a
    projected[root / row['source']] = old
    assert len(b.splitlines()) <= 1500
assert len(projected) == 2
source = root / 'src/cleanup/inventory_builder/typed.rs'
current = source.read_text(); original = projected[source].decode()
expected = original.replace('use receiving::{initial_controls, receiving, INITIAL_CONTROL_INITIALIZERS};', 'use receiving::{receiving, INITIAL_BYTES, INITIAL_CONTROL_INITIALIZERS};')
expected = expected.replace('    let bytes = initial_controls();\n    match physical.reserve(bytes) {', '    match physical.reserve(INITIAL_BYTES) {', 1)
assert current == expected
assert 'Err(_) => return Err(InventoryFailure(Kind::Receiving))' in current
receiving = (root / 'src/cleanup/inventory_builder/typed/receiving.rs').read_text()
assert 'fn initial_controls' not in receiving
assert "pub(super) const INITIAL_BYTES: usize = size_of::<Initial<'static>>();" in receiving
assert 'INITIAL_CONTROL_INITIALIZERS: usize = 42;' in receiving
for name,count in [('Controls',32),('Initial',42)]:
    body = re.search(r'type '+name+r"<[^=]+> = \((.*?)\n\);", receiving, re.S)[1]
    assert len([x for x in body.splitlines() if x.strip()]) == count
old_packet = 'docs/evidence/typed-raw-inventory-paid-boundary-781-next-20261016'
assert not subprocess.check_output(['git','diff',data['base'],'--',old_packet],cwd=root)
assert (packet / 'pending-selectors.json').read_bytes() == (root / old_packet / 'pending-selectors.json').read_bytes()
# Authenticate both complete raw inputs first, then replay the UNCHANGED prior
# source proof against its exact domain without writing or swapping source.
read_bytes,read_text,argv = Path.read_bytes,Path.read_text,sys.argv
try:
    def virtual_bytes(path):
        return projected[path.resolve()] if path.resolve() in projected else read_bytes(path)
    def virtual_text(path,*args,**kwargs):
        if path.resolve() in projected:
            return projected[path.resolve()].decode(kwargs.get('encoding') or (args[0] if args else None) or 'utf-8')
        return read_text(path,*args,**kwargs)
    Path.read_bytes,Path.read_text = virtual_bytes,virtual_text
    sys.argv = ['verify-source.py','--repo',str(root)]
    runpy.run_path(str(root / old_packet / 'verify-source.py'),run_name='__main__')
finally:
    Path.read_bytes,Path.read_text,sys.argv = read_bytes,read_text,argv
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_successor_inputs':2,'initial_fields':42,'common_fields_unchanged':32,'initial_failure_abi':'STILL_EXPLICIT_RESIDUAL','same_two_owners_pending':True,'rust_executed':False},sort_keys=True))
