#!/usr/bin/env python3
"""Existing full-source inverse protocol, extended for the private #854 join."""
import argparse,hashlib,json,pathlib,subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);a=ap.parse_args();r=pathlib.Path(a.repo).resolve();p=r/'docs/evidence/canonical-intern-arena-854-joined-20261016';component=r/'docs/evidence/canonical-intern-arena-854-next-20261016'
base='ab13bc4855ab9559e2e18725c4ce1147a5071bd0';parent='3c3886620dff92f0d5ae8f9e5a7b097e3c3c73ad';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(h,s):return subprocess.check_output(['git','show',h+':'+s],cwd=r)
def inverse(row,s):
 assert sha(s.encode())==row['current_sha256']
 for e in row['edits']:
  assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'];assert s.encode()==(r/row['original_archive']).read_bytes();return s
rows=json.loads((p/'current-to-ab13.json').read_text());assert len(rows)==8;physical={}
for row in rows:
 prior=inverse(row,(r/row['source']).read_text());assert prior.encode()==git(base,row['source']);physical[row['source']]=prior
# The entire earlier component capsule remains physically byte-exact.
paths=subprocess.check_output(['git','ls-tree','-r','--name-only',base,str(component.relative_to(r))],cwd=r,text=True).splitlines()
for s in paths:assert (r/s).read_bytes()==git(base,s),s
oldrows=json.loads((component/'current-to-3c388.json').read_text());assert len(oldrows)==3
for row in oldrows:
 old=inverse(row,physical.get(row['source'],(r/row['source']).read_text()));assert old.encode()==git(parent,row['source'])
old=git(parent,'src/cleanup_plan/replay/factored/canonical_shared.rs').decode();start=old.index('// Invoked only after preflight');end=old.index('\n#[cfg(test)]\nmod tests;');copy=(r/'src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs').read_text();assert old[start:end]in copy and 'intern_arena'not in copy
owners=json.loads((p/'new-owners.json').read_text());assert len(owners)==4
for row in owners:
 b=(r/row['source']).read_bytes();assert sha(b)==row['sha256'];assert len(b.decode().splitlines())==row['lines']<=1500
 for audit,anchor in [('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]:
  assert (r/audit).read_text().count('include_str!("'+anchor+row['source']+'")')==1
helper=(r/'src/source_reflows_merge.rs').read_text();assert helper.count('let prior = arena854_previous(source, current);')==2
assert 'let component = arena854_input(source, current);'in helper
# Complete original project algorithm, every old method and source reader are
# retained: their whole predecessor helper is one of the exact inverse rows.
for row in rows:
 if row['source'].endswith('.rs'):assert len((r/row['source']).read_text().splitlines())<=1500
exact=['memo.rs','memo/retained.rs','empty_memo.rs','empty_memo/retained.rs','closure_receiving.rs','ordered_weights.rs','receiving_layer_tests.rs','unit_receiving_tests.rs','tests.rs']
for s in exact:
 path='src/cleanup_plan/replay/factored/canonical_shared/'+s;assert (r/path).read_bytes()==git(parent,path)
assert len(json.loads((p/'pending-selectors.json').read_text()))==3
print(json.dumps({'status':'PASS_PRIVATE_JOIN_SOURCE_ONLY','component':base,'immediate_parent':parent,'successor_whole_rows':len(rows),'protected_component_whole_rows':len(oldrows),'all_component_artifacts_byte_exact':len(paths),'old_receiver_complete':True,'owners_joined_all_three':len(owners),'original_controls_exact':exact,'rust_executed':False,'gain_claim':False},indent=2))
