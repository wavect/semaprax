#!/usr/bin/env python3
"""Finite source-only proof; does not execute Rust or certify Session closure."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();root=a.repo.resolve()
packet=root/'docs/evidence/status-scalar-paid-receiving-781-next-20261016'
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
# Full original status and record producers remain untouched before adapters.
for path in ['src/cleanup_plan/replay/status_construction.rs','src/cleanup_plan/replay/scalar_replay/records.rs','src/cleanup_plan/replay/flat_records/tests.rs','src/cleanup_plan/renewal_profile.rs']:
    old=subprocess.check_output(['git','show',data['base']+':'+path],cwd=root)
    assert (root/path).read_bytes().startswith(old),path
def function(source,name):
    start=source.index('fn '+name+'(');return source[start:source.index('\n}',start)+2]
for driver in ['lexical_replay','scalar_replay']:
    path='src/cleanup_plan/replay/'+driver
    old=subprocess.check_output(['git','show',data['base']+':'+path+'.rs'],cwd=root).decode()
    before=(root/(path+'/before_status_scalar.rs')).read_text()
    assert function(old,'validate_paid')==function(before,'validate_paid')
    assert 'status_construction::paid::' not in before
    assert 'records::paid::' not in before
    if driver=='scalar_replay':
        assert function(old,'scalar_nodes')==function((root/(path+'.rs')).read_text(),'scalar_nodes')
# No status algorithms were dropped: both functions and the cases producer
# remain whole in the archived root, while twelve real primitive sites move.
status=(root/'src/cleanup_plan/replay/status_construction/paid.rs').read_text()
assert status.count('receive!(')==12
assert status.count('status semantic atom receiving')==2
lookup=status.index('.resolve_call_target(callee, instance.as_ref())')
assert lookup<status.index('status semantic atom receiving')<status.index('semantic_refusal::Atom::StatusUnknownCallee')
assert 'replay_error(' not in status
scalar=(root/'src/cleanup_plan/replay/scalar_replay/records/paid.rs').read_text()
assert 'replay_error(' not in scalar
assert '.map_err(|reason| permit.refuse' not in scalar
nodes=(root/'src/cleanup_plan/replay/scalar_replay/paid_nodes.rs').read_text()
assert 'budget.charge(' in nodes and '.map_err(Error::Owned)' in nodes
assert 'child_charge(' not in nodes
# Prior packet JSON and scripts are immutable. Do not claim that its source
# hashes can be checked against a different raw domain without an outer view.
prior='docs/evidence/structural-leaf-paid-receiving-781-next-20261016/'
for path in subprocess.check_output(['git','ls-tree','-r','--name-only',data['base'],prior],cwd=root).decode().splitlines():
    assert (root/path).read_bytes()==subprocess.check_output(['git','show',data['base']+':'+path],cwd=root)
for row in data['rows']:
    if row['source'].endswith('.rs'):
        source=root/row['source'];actual=len(source.read_text().splitlines())
        old_lines=len((packet/row['before_archive']).read_text().splitlines())
        assert actual<=1500 or actual<=old_lines,(row['source'],actual,old_lines)
for name in ['status_refusal','scalar_refusal']:
    assert '#[derive(Clone, Copy, Debug, Eq, PartialEq)]' in (root/('src/cleanup_plan/renewal_profile/'+name+'.rs')).read_text()
assert len(json.loads((packet/'pending-selectors.json').read_text()))==4
print(json.dumps({'status':'PASS_SOURCE_ONLY','full_before_inverse_rows':len(data['rows']),'new_source_owners':len(data['new_sources']),'independent_complete_old_drivers':2,'rust_executed':False,'issue_781':'OPEN_REMAINING_FAMILIES'},sort_keys=True))
