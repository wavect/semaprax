#!/usr/bin/env python3
"""Finite source-only custody proof. No Rust execution or measured gain claim."""
import argparse, hashlib, json, pathlib, subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);r=pathlib.Path(ap.parse_args().repo).resolve();base='364d824194cd1df76b5c273aa0fb9cbafb602e29';packet='docs/evidence/singleton-state-move-next-20261016/';p=r/packet
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(s):return subprocess.check_output(['git','show',base+':'+s],cwd=r)
rows=json.loads((p/'current-to-364d.json').read_text());assert len(rows)==1
row=rows[0];current=(r/row['source']).read_text();assert sha(current.encode())==row['current_sha256'];prior=current
for e in row['edits']:assert prior.count(e['new'])==1;prior=prior.replace(e['new'],e['old'],1)
assert prior.encode()==git(row['source'])==(r/row['original_archive']).read_bytes();assert sha(prior.encode())==row['original_sha256']
old=prior
for reg in json.loads((p/'old-registration-adapters.json').read_text()):assert old.count(reg)==1;old=old.replace(reg,'',1)
assert old==(r/'src/cleanup_plan/replay/path_join/before_state_move.rs').read_text()
start=prior.index('        let mut groups = BTreeMap::<ControlKey, BTreeSet<Box<PathState>>>::new();');end=prior.index('\n    if terminal_paths == 0',start)
assert current.count(prior[start:end])==1,'complete multistate body and schedule'
a=prior.index('            validate_control_group_with_budget(function, &states, block_id, budget)?;');b=prior.index('            if let Some(auth) = lexical {',a)
c=current.index('            validate_control_group_with_budget(function, &states, block_id, budget)?;');d=current.index('            if let Some(auth) = lexical {',c)
# Everything outside the selected singleton fragment and two child registrations
# is byte-exact, including validators, transition work, all edges and scheduling.
registration='\n#[cfg(test)]\nmod before_state_move;\n#[cfg(test)]\nmod state_move_tests;\n'
assert current.endswith(registration)
assert current[:c]==prior[:a];assert current[d:-len(registration)]==prior[b:]
fragment=current[c:d];assert 'joined_ownership_state(' not in fragment;assert 'clone(' not in fragment;assert 'state.live_order =' not in fragment
assert fragment.count('branch_joins.normalize(')==1
assert 'state.conditional_variants,\n                budget,' in fragment
# Removed census charges physical clone bytes only; it contains no work debit.
storage=(r/'src/cleanup_plan/replay/path_join/storage.rs').read_text();a=storage.index('pub(super) fn reserve_ownership_clone(');b=storage.index('pub(super) fn reserve_control_clone(',a);assert '.charge(' not in storage[a:b]
for s in ['src/cleanup_plan/replay/path_join/storage.rs','src/cleanup_plan/replay/path_join/branch_join.rs','src/cleanup_plan/replay/path_join/boxed_states.rs','src/cleanup_plan/replay/path_join/singleton_tests.rs','src/cleanup_plan/replay/path_join/branch_join/tests.rs','src/cleanup_plan/replay.rs']:
 assert (r/s).read_bytes()==git(s),s
owners=json.loads((p/'new-owners.json').read_text());assert len(owners)==2
for owner in owners:
 data=(r/owner['source']).read_bytes();assert sha(data)==owner['sha256'];assert len(data.splitlines())==owner['lines']<=1500
changed=subprocess.check_output(['git','diff','--name-only',base],cwd=r,text=True).splitlines();allowed={row['source']}|{o['source'] for o in owners};assert all(s in allowed or s.startswith(packet) for s in changed),changed
assert len(current.splitlines())<=1500
print(json.dumps({'status':'PASS_SOURCE_ONLY','base':base,'whole_current_to_git_inverse':1,'complete_compiled_old_receiver':'registration-only adapters','multistate_and_all_other_edges_exact':True,'shipping_sha256':row['current_sha256'],'new_owners':owners,'new_test_bodies':2,'canonical_global_front_and_three_audit_joins':'PENDING_COMPOSITION','rust_execution':False,'numerical_gain_claim':False},indent=2))
