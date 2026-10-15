from pathlib import Path
import hashlib,json,subprocess
r=Path.cwd();p=r/'docs/evidence/owning-corrections-940';sha=lambda x:hashlib.sha256(x).hexdigest()
rows=json.loads((p/'exact-source-inverses.json').read_text())
for row in rows:
 current=(r/row['source']).read_text()
 assert sha(current.encode())==row['current_sha256']
 for edit in row['edits']:
  assert current.count(edit['new'])==1
  current=current.replace(edit['new'],edit['before'],1)
 assert current==row['original_bytes']
 assert len((r/row['source']).read_text().splitlines())<=1500
q=r/'src/cleanup_plan/replay/factored/canonical_shared';saved=(q/'before_receiving_layers.rs').read_text();accessors='\nimpl Canonicalizer';saved=saved[:saved.rindex(accessors)]
registrations='#[cfg(test)]\nmod before_alias;\n#[cfg(test)]\nmod before_index;\n#[cfg(test)]\nmod before_targets;\nmod epsilon_alias;\nmod memo;\n#[cfg(test)]\nmod singleton_target_tests;\nmod successor_targets;\n'
restored=saved.replace('use super::super::{','use super::{',1).replace('use super::{app_census, epsilon_alias, memo, successor_targets, tests};\n',registrations,1)+'#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\npub(super) mod app_census;\n'
a=r/'docs/evidence/singleton-target-receiving-layers-831'
assert restored==(a/'complete-receiver-before.rs.txt').read_text()
assert (q/'singleton_target_tests.rs').read_bytes()==(a/'complete-seven-controls-before.rs.txt').read_bytes()
helper=(q/'successor_targets.rs').read_text().replace('#[cfg(test)]\nuse super::{','use super::{',1).replace('use std::collections::BTreeMap;\n#[cfg(test)]\nuse std::mem::size_of;','use std::{collections::BTreeMap, mem::size_of};',1).replace('// Complete prior helpers qualify old receiving boundaries only.\n#[cfg(test)]\nimpl Targets {','impl Targets {',1)
assert helper==(a/'complete-target-helpers-before.rs.txt').read_text()
root=r/'src/cleanup_plan/replay/expression_facts/receiving/regions_entry.rs';current=root.read_text();assert current==(p/'complete-regions-entry-current.rs.txt').read_text();assert sha(current.encode())=='f27fb4d28d71bb522bafb208a3da4bd8098ec61b5cca904f33d5d9858738b0e4'
for row in json.loads((r/'docs/evidence/region-entry-receiving-816/receiver-projections.json').read_text()):
 before=(r/'docs/evidence/region-entry-receiving-816'/row['original']).read_text();assert before==row['before'];assert sha(before.encode())==row['sha256']
 after=row['after'].replace('let under = validate_place(function, place, storage, leaves)?;','let under = helpers::validate_place(function, place, storage, leaves, budget)?;');assert current.count(after)==1
 assert (r/'src/cleanup_plan/replay.rs').read_text().count(before)==1
# The independent old six-field adapter is preserved verbatim, including both new child arms.
src='src/cleanup_plan/replay/semantic_refusal/tests.rs';old=subprocess.check_output(['git','show',rows[0]['basis']+':'+src],text=True);now=(r/src).read_text();start=old.index('fn complete_original_legacy(');end=old.index('\n#[test]',start);adapter = '        Atom::SelectedCall(kind) => kind.detail().to_owned(),\n        Atom::UnknownCallCallee(callee) => format!("call commit has unknown callee `{callee}`"),\n';assert now.count(adapter)==1;assert old[start:end] in now.replace(adapter,'',1)
for f in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 s=(r/f).read_text()
 for path in [row['source'] for row in rows[:6]]:assert s.count(path)==1,(f,path)
# No compiler/runtime gate is inferred from these text proofs.
proof={'status':'PASS_SOURCE_ONLY','whole_original_rows':len(rows),'owner_67':'complete receiver/helpers/seven controls exact','owner_108':'complete current module digest plus original independent projections exact','owner_104':'same original constructors for error policy; actual old/current constructor absolute debits compared independently; no guessed expected fee','owner_136':'unmodified allocation<=charged assertion; raw legacy oracle prepayment uses exact function authority; paid producer/report remains sealed data','all_six_field_original_adapter':'byte exact','rust_build':'UNEXECUTED','rust_tests':'UNEXECUTED','clippy':'UNEXECUTED'}
(p/'source-only-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
