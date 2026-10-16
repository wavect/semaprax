#!/usr/bin/env python3
"""Bounded source proof; does not execute Rust or claim current tests passed."""
import argparse, hashlib, json, pathlib, subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);r=pathlib.Path(ap.parse_args().repo).resolve()
base='364d824194cd1df76b5c273aa0fb9cbafb602e29';packet='docs/evidence/lexical-scratch-receiving-831-next-20261016/';p=r/packet
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(path):return subprocess.check_output(['git','show',base+':'+path],cwd=r)
rows=json.loads((p/'current-to-364d.json').read_text());assert len(rows)==2
for row in rows:
 text=(r/row['source']).read_text();assert sha(text.encode())==row['current_sha256'];old=text
 for e in row['edits']:assert old.count(e['new'])==1;old=old.replace(e['new'],e['old'],1)
 assert sha(old.encode())==row['original_sha256'];assert old.encode()==git(row['source'])==(r/row['original_archive']).read_bytes()
 assert len(text.splitlines())<=1500
changed=subprocess.check_output(['git','diff','--name-only',base],cwd=r,text=True).splitlines();assert all(s in {x['source'] for x in rows} or s.startswith(packet) for s in changed),changed
root=(r/rows[0]['source']).read_text();old=git(rows[0]['source']).decode()
assert 'fn prepare_scratch(' not in root and 'self.prepare_scratch(' not in root
assert root.count('"factored retained canonical scratch controls"')==2
assert old.count('"factored retained canonical scratch controls"')==1
assert '"factored closure memo authorization receipt"' not in root
memo='src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs';m=(r/memo).read_text();original=git(memo).decode()
assert m.replace('pub(super) function:', 'function:').replace('pub(super) budget:', 'budget:').replace('pub(super) ledger:', 'ledger:')==original
# Every other tracked blob/mode is exact, including complete independent old
# receivers, ordinary Memo/RetainedMemo, all prior JSON and every test assertion.
exact=[ 'src/cleanup_plan/replay/factored/canonical_shared/'+s for s in ['memo.rs','memo/retained.rs','before_unit.rs','before_unit/empty_memo.rs','before_unit/empty_memo/retained.rs','receiving_layer_tests.rs','unit_receiving_tests.rs','empty_closure_tests.rs','closure_receiving/tests.rs','closure_receiving.rs','strategy.rs'] if s!='strategy.rs']
for s in exact:assert (r/s).read_bytes()==git(s),s
assert (r/'src/cleanup_plan/replay/factored/strategy.rs').read_bytes()==git('src/cleanup_plan/replay/factored/strategy.rs')
assert root.count('activate_closure!(')==2
assert 'std::mem::size_of::<Option<empty_memo::Binding>>()' in root
print(json.dumps({'status':'PASS_SOURCE_ONLY','base':base,'whole_inverse_rows':2,'shipping_sources':{x['source']:x['current_sha256'] for x in rows},'all_other_tracked_sources_and_tests_exact':True,'new_test_bodies':0,'inherited_projection':'PENDING_CANONICAL_RAW_TO_364D_FRONT','rust_execution':False,'numerical_gain_claim':False},indent=2))
