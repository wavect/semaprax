#!/usr/bin/env python3
"""Finite source-only custody/authority proof. Never invokes Rust or a build."""
import argparse, hashlib, json, pathlib, subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);a=ap.parse_args();r=pathlib.Path(a.repo).resolve();base='2d4b68d3a416cae5fba6ac08498cd56721878ba1';p=r/'docs/evidence/closure-unit-receiving-831-next-20261016'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(path):return subprocess.check_output(['git','show',base+':'+path],cwd=r)
rows=json.loads((p/'current-to-2d4b.json').read_text());assert len(rows)==9
for row in rows:
 current=(r/row['source']).read_text();assert sha(current.encode())==row['current_sha256'];old=current
 for edit in row['edits']:
  assert old.count(edit['new'])==1;old=old.replace(edit['new'],edit['old'],1)
 assert sha(old.encode())==row['original_sha256'];assert old.encode()==git(row['source']);assert old.encode()==(r/row['original_archive']).read_bytes()
b=r/'src/cleanup_plan/replay/factored/canonical_shared';old=git('src/cleanup_plan/replay/factored/canonical_shared.rs').decode();copy=(b/'before_unit.rs').read_text();start=old.index('type SharedWeighted =');end=old.index('\n#[cfg(test)]\nmod tests;',start);body=old[start:end];i=copy.index('type SharedWeighted =');assert copy[i:i+len(body)]==body
for suffix,source in [('empty_memo.rs','empty_memo.rs'),('empty_memo/retained.rs','empty_memo/retained.rs')]:assert (b/'before_unit'/suffix).read_bytes()==git('src/cleanup_plan/replay/factored/canonical_shared/'+source)
oldslot=git('src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs').decode();cut=oldslot.index('\n#[cfg(test)]\npub(in crate::cleanup_plan::replay) mod source;');assert (b/'before_unit/closure_receiving.rs').read_text()==oldslot[:cut]+'\n'
# All ordinary memo owners, original strict control bodies, old immutable runtime
# tables, and shared map/value representations remain byte-exact.
exact=['src/cleanup_plan/replay/factored/canonical_shared/'+s for s in ['memo.rs','memo/retained.rs','receiving_layer_tests.rs','empty_closure_tests.rs','closure_receiving/tests.rs','inline_weighted.rs','ordered_weights.rs','empty_memo/retained.rs']]
for source in exact:assert (r/source).read_bytes()==git(source),source
changed=subprocess.check_output(['git','diff','--name-only',base],cwd=r,text=True).splitlines();allowed={row['source'] for row in rows}
new_paths={owner['source'] for owner in json.loads((p/'new-owners.json').read_text())}
extra=[source for source in changed if source not in allowed and source not in new_paths and not source.startswith('docs/evidence/closure-unit-receiving-831-next-20261016/')]
assert not extra,extra
# Existing projector transform stays exact, with a strict new stage before the
# old compiler-fix stage. No old stage accepts already-projected input.
helper=(r/'src/source_reflows_merge.rs').read_text();prior=git('src/source_reflows_merge.rs').decode()
def project(s):return s[s.index('fn project('):s.index('// Exact post-build name/visibility repair,')]
assert project(helper)==project(prior)
assert helper.count('let unit = unit831_input(source, current);')==1
# Preserve every original full audit input. New children are independently
# registered under cfg(test), and all three raw audits join all five whole files.
owners=json.loads((p/'new-owners.json').read_text());assert len(owners)==5
for owner in owners:
 path=r/owner['source'];assert sha(path.read_bytes())==owner['sha256'];assert len(path.read_text().splitlines())==owner['lines']<=1500
 for audit in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
  text=(r/audit).read_text();assert text.count(owner['source'])==1,(audit,owner['source'])
for source in allowed:
 if source.endswith('.rs'):assert len((r/source).read_text().splitlines())<=1500,source
assert '#[cfg(test)]\nmod before_unit;' in (r/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_text()
assert '#[cfg(test)]\nmod unit_receiving_tests;' in (r/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_text()
assert '#[path = "before_unit/empty_memo.rs"]' in copy
assert '#[path = "before_unit/closure_receiving.rs"]' in copy
assert '#[path = "empty_memo/retained.rs"]' in (b/'before_unit/empty_memo.rs').read_text()
# The only runtime shipping changes are these three preexisting production files;
# every other changed Rust owner is source-only projection/audit scaffolding.
shipping=[s for s in allowed if s.startswith('src/cleanup_plan/')]
assert sorted(shipping)==sorted('src/cleanup_plan/replay/factored/canonical_shared'+s for s in ['.rs','/closure_receiving.rs','/empty_memo.rs'])
print(json.dumps({'status':'PASS_SOURCE_ONLY','base':base,'whole_input_rows':len(rows),'independent_new_owners':len(owners),'ordinary_and_strict_exact':exact,'shipping_sources':{s:sha((r/s).read_bytes()) for s in sorted(shipping)},'rust_execution':False,'gain_claim':False},indent=2))
