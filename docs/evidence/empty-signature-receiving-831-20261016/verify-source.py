#!/usr/bin/env python3
"""Exact finite source inverses and independently compiled complete old receiver."""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);a=p.parse_args();r=a.repo.resolve();assert r.is_relative_to('/Volumes/Semaprax');d=r/'docs/evidence/empty-signature-receiving-831-20261016';base='d96247b3cb69282c3d1b835b2428779e8bde82c7';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(path):return subprocess.check_output(['git','show',base+':'+path],cwd=r)
def inverse(row,source):
 assert sha(source.encode())==row['current_sha256'],'whole actual input drift'
 for edit in row['edits']:
  assert edit['new'] and source.count(edit['new'])==1,'missing/duplicate exact inverse fragment';source=source.replace(edit['new'],edit['old'],1)
 assert sha(source.encode())==row['original_sha256'];assert source.encode()==(r/row['original_archive']).read_bytes()==git(row['source']);return source
rows=json.loads((d/'current-to-d962.json').read_text());expected={'CHANGELOG.md','docs/ARCHITECTURE.md','docs/FACTORED-SINGLETON-TARGETS-V1.md','src/cleanup_plan/replay/factored/canonical_shared.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs'};assert {x['source']for x in rows}==expected and len(rows)==len(expected)
negatives=0
for row in rows:
 current=(r/row['source']).read_text();inverse(row,current)
 for changed in [current+'\n',current.replace(row['edits'][0]['new'],'',1)]:
  try:inverse(row,changed)
  except AssertionError:negatives+=1
  else:raise AssertionError('altered finite input accepted')
 if row['source'].endswith('.rs'):assert len(current.splitlines())<=1500
contract=json.loads((d/'source-contract.json').read_text());old=git(contract['source']).decode();assert sha(old.encode())==contract['old_sha256'] and old==(d/contract['archive']).read_text();body=old[old.index(contract['body_start']):old.index(contract['body_end'],old.index(contract['body_start']))];cold=(r/contract['compiled_before']).read_text();assert cold==contract['compiled_prefix']+body+contract['compiled_suffix'];assert 'empty_signature::'not in body
root=(r/contract['source']).read_text();assert root.count(contract['inserted_block'])==1;assert root.removesuffix(contract['appended_registrations']).replace(contract['inserted_block'],'',1)==old
# The complete old source outside the one new lexical block/registrations is
# byte-exact, including every preflight, work request, fallback and refusal.
assert root.index('"factored canonical signature construction"')<root.index(contract['inserted_block']);assert root.index('"factored canonical signature vector"',root.index(contract['inserted_block']))<root.index('let id = match self.ids.get(&key)',root.index(contract['inserted_block']))
owners=json.loads((d/'new-owners.json').read_text());assert len(owners)==3
for row in owners:
 actual=(r/row['source']).read_bytes();assert sha(actual)==row['sha256'] and len(actual.decode().splitlines())==row['lines']<=1500
 for guard,anchor in [('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]:assert (r/guard).read_text().count('include_str!("'+anchor+row['source']+'")')==1
native='crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs';assert len((r/native).read_text().splitlines())==1500
prefix='src/cleanup_plan/replay/factored/canonical_shared/'
protected=['receiving_layer_tests.rs','singleton_target_tests.rs','successor_targets.rs','before_receiving_layers.rs','inline_weighted.rs','inline_weighted/tests.rs','intern_arena.rs','arena_tests.rs','arena_source.rs','closure_receiving.rs','empty_memo.rs','empty_memo/retained.rs','ordered_weights.rs']
for path in protected:assert (r/prefix/path).read_bytes()==git(prefix+path),path
proof={'status':'PASS_SOURCE_ONLY_PRIVATE_COMPONENT_NOT_FINAL_JOINED_ENDPOINTS','baseline':base,'whole_preimages_and_inverses':len(rows),'hostile_current_input_drifts_refused':negatives,'complete_immediate_body_verbatim_and_independent':True,'general_body_and_all_existing_guards_work_refusal_bytes_preserved':True,'old_strict_paid_tests_and_all_protected_controls_byte_exact':protected,'new_helpers_bound_in_all_three_raw_joins':3,'native_lines':1500,'new_owners_pending':4,'final_joined_input_bridge_and_endpoint_refresh':'PENDING_PARENT_NEXT_INTEGRATION','rust_executed':False,'gain_claim':False};(d/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
