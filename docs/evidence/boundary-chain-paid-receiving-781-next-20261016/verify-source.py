#!/usr/bin/env python3
"""Finite source-only proof; does not execute Rust or certify Session closure."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();root=a.repo.resolve()
packet=root/'docs/evidence/boundary-chain-paid-receiving-781-next-20261016'
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
for tail,new in [('paid.rs','before_boundary.rs'),('paid/storage.rs','before_boundary/storage.rs')]:
    path='src/cleanup_plan/replay/lexical_cert/'
    assert (root/(path+new)).read_bytes()==original(path+tail)
def function(source,name):
    start=source.index('fn '+name+'(');return source[start:source.index('\n}',start)+2]
path='src/cleanup_plan/replay/lexical_replay'
a=function(original(path+'.rs').decode(),'validate_paid')
b=function((root/(path+'/before_boundary.rs')).read_text(),'validate_paid')
assert a.replace('lexical_cert::paid::authenticate(','lexical_cert::before_boundary::authenticate(')==b
for prefix in ['structural-leaf','status-scalar','certificate-precision']:
    prior='docs/evidence/'+prefix+'-paid-receiving-781-next-20261016/'
    for path in subprocess.check_output(['git','ls-tree','-r','--name-only',data['base'],prior],cwd=root).decode().splitlines():
        assert (root/path).read_bytes()==original(path)
for row in data['rows']:
    if row['source'].endswith('.rs'):
        n=len((root/row['source']).read_text().splitlines());old=len((packet/row['before_archive']).read_text().splitlines())
        assert n<=1500 or n<=old,(row['source'],n,old)
boundary=(root/'src/cleanup_plan/replay/lexical_boundary/paid.rs').read_text()
assert 'let mut admit =' not in boundary and 'macro_rules! admit' in boundary
assert 'replay_error(' not in boundary and 'Permit::acquire::<' in boundary
assert boundary.count('admit!(')==5
cert=(root/'src/cleanup_plan/replay/lexical_cert/paid.rs').read_text()
a=cert.index('let typed_ensures_scope');b=cert.index('if !is_return(entry)',a)
assert 'crate::cleanup_plan::replay::lexical_cert::storage::chain_contains(' in cert[a:b]
assert '.unwrap_or(false)' in cert[a:b] and 'budget.selected()' in cert[a:b]
assert 'lexical_boundary::paid::authenticate' in cert
storage=(root/'src/cleanup_plan/replay/lexical_cert/paid/storage.rs').read_text()
assert 'remaining.checked_sub(1)' in storage
assert 'certificate region chain' in storage and 'child_reserve_array::<CleanupRegionId>' in storage
assert len(json.loads((packet/'pending-selectors.json').read_text()))==4
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_before_inverse_rows':len(data['rows']),'new_sources':len(data['new_sources']),'independent_prior_certificate_storage_driver':True,'rust_executed':False,'issue_781':'OPEN_RESIDUAL_DESCENDANTS'},sort_keys=True))
