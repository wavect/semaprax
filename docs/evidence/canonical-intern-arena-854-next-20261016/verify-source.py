#!/usr/bin/env python3
"""Finite source-only component proof. No Rust/build/test invocation."""
import argparse,hashlib,json,pathlib,subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);a=ap.parse_args();r=pathlib.Path(a.repo).resolve()
p=r/'docs/evidence/canonical-intern-arena-854-next-20261016';base='3c3886620dff92f0d5ae8f9e5a7b097e3c3c73ad'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(s):return subprocess.check_output(['git','show',base+':'+s],cwd=r)
rows=json.loads((p/'current-to-3c388.json').read_text());assert len(rows)==3
for row in rows:
 current=(r/row['source']).read_text();assert sha(current.encode())==row['current_sha256'];old=current
 for edit in row['edits']:
  assert old.count(edit['new'])==1;old=old.replace(edit['new'],edit['old'],1)
 assert sha(old.encode())==row['original_sha256'];assert old.encode()==git(row['source']);assert old.encode()==(r/row['original_archive']).read_bytes()
b=r/'src/cleanup_plan/replay/factored/canonical_shared';old=git('src/cleanup_plan/replay/factored/canonical_shared.rs').decode();copy=(b/'before_arena.rs').read_text()
start=old.index('// Invoked only after preflight');end=old.index('\n#[cfg(test)]\nmod tests;')
assert old[start:end] in copy
assert copy[copy.index('// Invoked only after preflight'):copy.index('// Test-only observation;')].rstrip()==old[start:end].rstrip()
assert 'states: BTreeMap<SharedWeighted, usize>' in copy and 'ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>' in copy
assert 'intern_arena' not in copy
owners=json.loads((p/'new-owners.json').read_text());assert len(owners)==3
for owner in owners:
 data=(r/owner['source']).read_bytes();assert sha(data)==owner['sha256'];assert len(data.decode().splitlines())==owner['lines']<=1500
# Preserve ordinary memo/retained owners, old strict and all other control bodies.
exact=['memo.rs','memo/retained.rs','empty_memo.rs','empty_memo/retained.rs','closure_receiving.rs','ordered_weights.rs','receiving_layer_tests.rs','unit_receiving_tests.rs','tests.rs']
for s in exact:assert (b/s).read_bytes()==git(str((b/s).relative_to(r)))
root=(b.parent/'canonical_shared.rs').read_text();arena=(b/'intern_arena.rs').read_text();factored=(b.parent.parent/'factored.rs').read_text()
assert root.index('let Some(id) = self.states.get(child)')<root.index('let query = intern_arena::SignatureQuery')<root.index('match self.ids.get(&query)')<root.index('let mut key = Vec::with_capacity(successors.len());')
assert 'self.child_ids.push(*id);' in root
assert 'reserve_btree_node' not in root[root.index('CanonicalFrame::Exit(state, accepted, successors) => {'):]
assert factored.index('canonical_shared::Canonicalizer::CONSTRUCTOR_CONTROLS')<factored.index('let mut canonical = canonical_shared::Canonicalizer::new')
assert '#[cfg(test)]\nmod before_arena;' in root and '#[cfg(test)]\nmod arena_tests;' in root
assert 'unsafe' not in arena and 'Result<' not in arena[arena.index('fn rotate_left'):arena.index('#[cfg(test)]',arena.index('fn rotate_left'))]
for s in [b.parent/'canonical_shared.rs',b.parent.parent/'factored.rs',b/'inline_weighted.rs']:assert len(s.read_text().splitlines())<=1500
print(json.dumps({'status':'PASS_COMPONENT_SOURCE_ONLY_NOT_INTEGRATED','base':base,'whole_input_rows':3,'new_owners':3,'old_receiver_complete':True,'ordinary_owners_and_old_controls_exact':exact,'global_authority_fronts_pending':True,'rust_execution':False,'gain_claim':False},indent=2))
