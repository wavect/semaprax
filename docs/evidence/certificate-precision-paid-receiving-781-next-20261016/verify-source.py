#!/usr/bin/env python3
"""Finite source-only proof; does not execute Rust or certify Session closure."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();root=a.repo.resolve()
packet=root/'docs/evidence/certificate-precision-paid-receiving-781-next-20261016'
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

def function(source,name):
    start=source.index('fn '+name+'(');return source[start:source.index('\n}',start)+2]
for driver in ['lexical_replay','scalar_replay']:
    path='src/cleanup_plan/replay/'+driver
    old=subprocess.check_output(['git','show',data['base']+':'+path+'.rs'],cwd=root).decode()
    before=(root/(path+'/before_certificate.rs')).read_text()
    assert function(old,'validate_paid')==function(before,'validate_paid')
for tail in ['paid_nodes.rs','records/paid.rs']:
    path='src/cleanup_plan/replay/scalar_replay/'+tail
    assert (root/('src/cleanup_plan/replay/scalar_replay/before_certificate/'+tail)).read_bytes()==subprocess.check_output(['git','show',data['base']+':'+path],cwd=root)
for packet_name in ['structural-leaf-paid-receiving-781-next-20261016','status-scalar-paid-receiving-781-next-20261016']:
    prior='docs/evidence/'+packet_name+'/'
    for path in subprocess.check_output(['git','ls-tree','-r','--name-only',data['base'],prior],cwd=root).decode().splitlines():
        assert (root/path).read_bytes()==subprocess.check_output(['git','show',data['base']+':'+path],cwd=root)
for row in data['rows']:
    if row['source'].endswith('.rs'):
        n=len((root/row['source']).read_text().splitlines());old=len((packet/row['before_archive']).read_text().splitlines())
        assert n<=1500 or n<=old,(row['source'],n,old)
records=(root/'src/cleanup_plan/replay/scalar_replay/records/paid.rs').read_text()
assert records.count('Permit::acquire::<')==5 and 'Permit::profile::<' not in records
assert 'Permit::profile::<' in (root/'src/cleanup_plan/replay/scalar_replay/paid_nodes.rs').read_text()
cert=(root/'src/cleanup_plan/replay/lexical_cert/paid.rs').read_text()
assert 'replay_error(' not in cert and 'Permit::acquire::<()>' in cert
assert 'lexical_boundary::authenticate' in cert and 'type_needs_drop(' in cert
assert len(json.loads((packet/'pending-selectors.json').read_text()))==4
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_before_inverse_rows':len(data['rows']),'new_sources':len(data['new_sources']),'old_drivers':2,'rust_executed':False,'issue_781':'OPEN_RESIDUAL_DESCENDANTS'},sort_keys=True))
