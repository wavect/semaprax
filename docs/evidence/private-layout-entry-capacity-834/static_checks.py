"""Source proof only. No compiler, Cargo, Rust test or allocation observation."""
from pathlib import Path
import json,hashlib,subprocess
R=Path.cwd();D=R/'docs/evidence/private-layout-entry-capacity-834';BASE='940fcf220e47d1975f757e76f0b3401691db9358'
sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
rows=json.loads((D/'exact-source-inverses.json').read_text())
for row in rows:
 value=(R/row['source']).read_text();assert sha(value)==row['current_sha256'],row['source']
 for edit in row['edits']:
  assert value.count(edit['new'])==1,row['source'];value=value.replace(edit['new'],edit['before'],1)
 assert value==row['original_bytes'] and sha(value)==row['original_sha256'],row['source']
 assert len((R/row['source']).read_text().splitlines())<=1500,row['source']
root='src/workspace_graph/expected_projection/private_layout.rs';old=(D/'original'/ (root.replace('/','__')+'.before.txt')).read_text();current=(R/root).read_text()
register='''
#[cfg(test)]
#[path = "private_layout/capacity_before.rs"]
mod capacity_before;
#[cfg(test)]
#[path = "private_layout/capacity_tests.rs"]
mod capacity_tests;
'''
assert current.count('        let capacity = capacity.min(count);\n')==1
assert current.removesuffix(register).replace('        let capacity = capacity.min(count);\n','',1)==old
start=old.index('    fn derive_with_work(');end=old.index('    pub(super) fn add_costs(',start)
method=old[start:end].replace('    fn derive_with_work(','    pub(super) fn derive_with_work_before_capacity(',1)
assert (R/'src/workspace_graph/expected_projection/private_layout/capacity_before.rs').read_text()=="//! Complete preceding cold receiver; only method visibility/name differ.\nuse super::{Absence, AuthoredDeclaration, BTreeMap, DEPTH, Diagnostic, Entry, Frame, LayoutClosure, Program, debit, explicit_only, ledger, names, owner_index, private_name, refusal, reserve_builder_structure, sorting, visit};\nimpl<'a> LayoutClosure<'a> {\n"+method+'}\n'
# All count/name/overflow/work/alias/dedup/sort/owner checks stay byte-exact;
# only the existing reservation/backing/guard capacity is narrowed after fold.
assert current.index('let mut ordinal = names::first(caller)?')<current.index('debit(remaining, programs.len())?')<current.index('let capacity = programs')<current.index('let capacity = capacity.min(count)')<current.index('let mut entries: Vec<Entry')
helpers=['owner_index.rs','names.rs','roots.rs','shape.rs','sorting.rs','authority.rs','registry.rs','ledger.rs']
untouched={}
for name in helpers:
 path='src/workspace_graph/expected_projection/private_layout/'+name
 original=subprocess.check_output(['git','show',BASE+':'+path],text=True)
 assert (R/path).read_text()==original,path;untouched[path]=sha(original)
parent='src/workspace_graph/expected_projection.rs';assert (R/parent).read_bytes()==subprocess.check_output(['git','show',BASE+':'+parent])
new=[row['source']for row in rows if row['original_bytes']=='']
for file,prefix in [('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]:
 text=(R/file).read_text()
 for name in new+[root]:assert text.count('"'+prefix+name+'"')==1,(file,name)
proof={'status':'SOURCE_ONLY_NO_COMPILER_OR_TEST_EXECUTION','basis':BASE,'whole_original_rows_reconstructed':len(rows),'complete_old_receiver':'full_derive_with_work_body_exact_after_visibility_name_adapter_cfg_test_compilation_pending','complete_old_module_sha256':sha(old),'production_change':'one_post_fold_capacity_min_shared_by_real_reservation_backing_and_guard','original_checked_fold_name_work_alias_dedup_sort_owner_order':'byte_exact','occurrence_to_insertion_bound':'at_most_one_insert_per_same_immutable_callback; duplicate_full_stable_id_returns_zero','owner_index_bound':'UNCHANGED; imported_descendants_excluded_from_entry_count_are_not_capped_by_it','unchanged_helper_sha256':untouched,'recorded_1670_line_parent_unchanged':True,'full_source_joins':3,'cargo':'UNEXECUTED','rust_tests':'UNEXECUTED','runtime':'UNEXECUTED','allocator_observations':'AUTHORED_ONLY_UNEXECUTED','clippy':'UNEXECUTED','issue_834':'OPEN','measured_gain':'NOT_ESTABLISHED'}
(D/'source-only-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items()if k!='unchanged_helper_sha256'}))
