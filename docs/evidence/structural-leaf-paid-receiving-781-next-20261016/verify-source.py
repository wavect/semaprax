#!/usr/bin/env python3
"""Finite source-only proof; does not execute Rust or certify Session closure."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();root=a.repo.resolve()
packet=root/'docs/evidence/structural-leaf-paid-receiving-781-next-20261016'
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
# Every original algorithm in append-only roots remains whole and exact.
for path in ['src/cleanup_plan/renewal_profile.rs','src/cleanup_plan/replay/flat_records.rs','src/cleanup_plan/replay/leaf_index.rs','src/cleanup_plan/replay/structural_maps.rs','src/cleanup_plan/replay/selected_context/child.rs','src/cleanup_plan/replay/flat_records/tests.rs']:
    old=subprocess.check_output(['git','show',data['base']+':'+path],cwd=root)
    assert (root/path).read_bytes().startswith(old),path
# Both complete old paid drivers still dispatch to original structural children.
def function(source):
    start=source.index('fn validate_paid(');return source[start:source.index('\n}',start)+2]
for driver in ['lexical_replay','scalar_replay']:
    path='src/cleanup_plan/replay/'+driver
    old=subprocess.check_output(['git','show',data['base']+':'+path+'.rs'],cwd=root).decode()
    before=(root/(path+'/before_owned_families.rs')).read_text()
    assert function(old)==function(before)
    assert 'structural_maps::paid::' not in before
    shipping=(root/(path+'.rs')).read_text()
    for name in ['slots','leaves','regions']:
        assert function(shipping).count('structural_maps::paid::'+name+'(')==1
for row in data['rows']:
    if row['source'].endswith('.rs'):
        source=root/row['source'];actual=len(source.read_text().splitlines())
        old_lines=len((packet/row['before_archive']).read_text().splitlines())
        assert actual<=1500 or actual<=old_lines,(row['source'],actual,old_lines)
assert len(json.loads((packet/'pending-selectors.json').read_text()))==4
print(json.dumps({'status':'PASS_SOURCE_ONLY','full_before_inverse_rows':len(data['rows']),'new_source_owners':len(data['new_sources']),'independent_complete_old_drivers':2,'rust_executed':False,'issue_781':'OPEN_REMAINING_FAMILIES'},sort_keys=True))
