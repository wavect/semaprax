#!/usr/bin/env python3
"""Finite whole-source inverse and independent old receiver proof; no Rust."""
from pathlib import Path
import hashlib,json,subprocess,sys
repo=Path(sys.argv[1] if len(sys.argv)>1 else '.').resolve()
p=repo/'docs/evidence/branch-inline-receiving-next-808-20261016'
m=json.loads((p/'source-inverses.json').read_text()); sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
for row in m['sources']:
 before=subprocess.check_output(['git','show',m['base']+':'+row['path']],cwd=repo).decode()
 assert before==row['before']==(p/row['archive']).read_text()
 current=(repo/row['path']).read_text(); assert sha(current)==row['current_sha256']; assert sha(before)==row['before_sha256']
 restored=current
 for edit in reversed(row['edits']):
  assert restored.count(edit['after'])==1
  restored=restored.replace(edit['after'],edit['before'],1)
 assert restored==before
for row in m['additions']:assert sha((repo/row['path']).read_text())==row['sha256']
root=m['sources'][0]['before']; start=root.index('fn plan_skeleton_paths('); end=root.index('\nfn ',start+1)
fn=root[start:end]; assert fn==(p/'plan_skeleton_paths.before.rs.txt').read_text()
prefix='//! Complete immediate old enumerator, using its unchanged fused helper.\nuse super::super::*;\nuse super::super::branch_clone;\n\n'
assert (repo/'src/cleanup_plan/replay/branch_clone/before_inline.rs').read_text()==prefix+fn.replace('fn plan_skeleton_paths(','pub(super) fn plan_skeleton_paths(',1)
helper=(repo/'src/cleanup_plan/replay/branch_clone.rs').read_text()
for archive in ['controls.before.rs.txt','with_observation.before.rs.txt']:assert helper.count((p/archive).read_text())==1
# Existing old Main, original controls and all other production budget operations stay exact.
for path in ['src/cleanup_plan/replay/branch_clone/before.rs','src/cleanup_plan/replay/branch_clone/tests.rs','src/cleanup_plan/replay/branch_clone/funded_tests.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay/allocation.rs']:
 assert (repo/path).read_bytes()==subprocess.check_output(['git','show',m['base']+':'+path],cwd=repo)
actual=(repo/'src/cleanup_plan/replay.rs').read_text()
a=actual.index('fn plan_skeleton_paths(');b=actual.index('\nfn ',a+1)
assert actual[:a]==root[:start] and actual[b:]==root[end:]
assert actual[a:b].count('branch_clone::inline_controls')==1
assert 'branch_clone::with_observation(' not in actual[a:b]
assert actual.count('branch = Vec::with_capacity(capacity);')==1
assert len(actual.splitlines())<=7993
for row in m['additions']:assert len((repo/row['path']).read_text().splitlines())<=1500
print(json.dumps({'status':'PASS_SOURCE_ONLY_NO_RUST','base':m['base'],'full_existing_inverses':len(m['sources']),'full_new_owners':len(m['additions']),'old_main_and_budget_inputs_exact':5,'new_pending_selectors':2},indent=2))
