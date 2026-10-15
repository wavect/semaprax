"""Source-only checkpoint checks. This program does not compile or execute Rust."""
from pathlib import Path
import json,hashlib
R=Path.cwd();D=R/'docs/evidence/selected-call-refusal-781'
rows=json.loads((D/'exact-source-inverses.json').read_text())
for row in rows:
 current=(R/row['source']).read_text()
 assert hashlib.sha256(current.encode()).hexdigest()==row['current_sha256'],row['source']
 for edit in row['edits']:
  assert current.count(edit['new'])==1,row['source']
  current=current.replace(edit['new'],edit['before'],1)
 assert current==row['original_bytes'],row['source']
 assert hashlib.sha256(current.encode()).hexdigest()==row['original_sha256'],row['source']
C=R/'src/cleanup_plan/replay/structural_control'
old=(C/'calls_before_receipt.rs').read_text()
for before,after in [('pub(super) enum Signature<','enum Signature<'),('pub(super) fn signature<','fn signature<'),('    pub(super) fn parameter(','    fn parameter('),('    pub(super) fn accepts_type(','    fn accepts_type('),('    pub(super) fn len(','    fn len(')]:
 assert old.count(before)==1;old=old.replace(before,after,1)
assert old==(D/'original/src__cleanup_plan__replay__structural_control__calls.rs.before.txt').read_text()
old=(C/'blocks_before_receipt.rs').read_text().replace('use super::*;\nuse super::calls_before_receipt as calls;','use super::*;',1)
assert old==(D/'original/src__cleanup_plan__replay__structural_control__blocks.rs.before.txt').read_text()
new=[row['source']for row in rows if row['original_bytes']=='']
joined=new+['src/cleanup_plan/replay/structural_control.rs','src/cleanup_plan/replay/structural_control/blocks.rs','src/cleanup_plan/replay/structural_control/calls.rs']
for file,prefix in [('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]:
 text=(R/file).read_text()
 for name in joined:assert text.count('"'+prefix+name+'"')==1,(file,name)
for row in rows:
 name=row['source'];assert len((R/name).read_text().splitlines())<=1500,name
inventory=json.loads((D/'review-profile/literal-inventory.json').read_text())
assert len(inventory['static'])==19
receiver=(C/'call_receiving.rs').read_text();calls=(C/'calls.rs').read_text()
for label,kind in inventory['static'].items():assert 'Kind::'+kind in calls and '"'+label+'"'in receiver
assert calls.index('E::borrowed_controls(function, budget)?')<calls.index('program.borrowed_helper_lease(function)')<calls.index('lease.call_by_id(')
proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','basis':'7623acd670f0a25a21ed8c00c190b517e78c8945','whole_original_rows_reconstructed':len(rows),'complete_old_calls_and_blocks':'byte_exact_after_visibility_import_adapters','complete_source_joins':3,'finite_call_labels':19,'dynamic_labels':1,'borrowed_controls_precede_child':'source_asserted','affected_modules_at_most_1500_lines':True,'rust_compilation':'UNEXECUTED','rust_tests':'UNEXECUTED','runtime':'UNEXECUTED','whole_781':'OPEN'}
(D/'source-only-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
