"""Complete source-only custody. Does not compile/execute Rust or claim qualification."""
from pathlib import Path
import hashlib,json
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
packet=json.loads((D/'source-inverses.json').read_text());rows=packet['owners']
for row in rows+json.loads((D/'documentation-inverses.json').read_text()):
 current=(R/row['source']).read_bytes();old=(R/row['archive']).read_bytes()
 assert sha(current)==row['current_sha256'] and sha(old)==row['original_sha256']
 text=current.decode()
 for e in row['edits']:
  assert text.count(e['new'])==1;text=text.replace(e['new'],e['before'],1)
 assert text.encode()==old
for child in packet['children']:assert sha((R/child['source']).read_bytes())==child['sha256']
old=(D/'original/workspace_graph.rs').read_text()
a=old.index('fn build_owned_inner(');b=old.index('\nfn build_resolved_core_current(',a);body=old[a:b].replace('fn build_owned_inner(','pub(super) fn build_owned_inner(',1)
a=old.index('fn build_owned_retaining_sources_with_frontend_limit(');b=old.index('\nfn active_builder_limit()',a);wrapper=old[a:b].replace('fn build_owned_retaining_sources_with_frontend_limit(','pub(super) fn build_owned_retaining_sources_with_frontend_limit(',1)
assert (R/'src/workspace_graph/cold_canonical/before.rs').read_text()=='//! Complete immutable940 cold preflight; no comparison-sink delegation.\nuse super::super::*;\n'+body+'\n'+wrapper
assert (R/'src/format.rs').read_bytes()==(D/'original/format.rs').read_bytes()
previous=(D/'original/comments.rs').read_text()
comment_row=next(row for row in rows if row['source']=='src/format/comments.rs')
comments=next(edit['new'] for edit in comment_row['edits'] if edit['before']==previous)
registration='#[path = "comments/compare.rs"]\nmod compare;\npub(crate) use compare::canonical_comparison;\n'
assert comments.replace(registration,'',1)==previous,'complete old placement/String formatter unchanged'
root_row=next(row for row in rows if row['source']=='src/workspace_graph.rs')
root=next(edit['new'] for edit in root_row['edits'] if edit['before']==old)
restored=root.replace('#[cfg(test)]\nmod cold_canonical;\n','',1).replace('format::comments::canonical_comparison(&program, comments, &source.source)','format::comments::canonical_with_comments(&program, comments)',1).replace('if overflowed || canonical.bytes.is_none() {','if overflowed {',1).replace('if !canonical.matches {','if canonical != source.source {',1).replace('canonical.bytes.expect("canonical byte count checked before source equality")','canonical.len()',1)
assert restored==old,'no constructor loop/other production root changes'
assert root.index('if overflowed || canonical.bytes.is_none() {')<root.index('if !canonical.matches {')
compare=(R/'src/format/comments/compare.rs').read_text()
for text in ['total.checked_add(text.len())','self.offset.checked_add(text.len())','self.mismatch = true;','Ok(())','super::super::write_canonical_commented(program, &placement, &mut output);','#[path = "compare/tests.rs"]']:assert text in compare
assert 'String::'not in compare and 'Vec::'not in compare
inventory=(R/'tests/workspace/semantic_graph/source_inventory.rs').read_text()
for child in packet['children']:assert child['source']in inventory,('missing full source audit join',child['source'])
pending=json.loads((D/'pending-selectors.json').read_text());assert pending['new']==8 and pending['required_existing']==14
for row in pending['checks']:
 assert sha((R/row['source']).read_bytes())==row['source_sha256']
 assert not row['source_binding']['ignored']
 assert 'fn '+row['source_binding']['name']+'('in (R/row['source']).read_text()
proof={'status':'SOURCE_ONLY_UNEXECUTED_NO_COMPILER_OR_RUNTIME','basis':packet['basis'],'whole_changed_source_owners':len(rows),'documentation_originals':2,'complete_old_builder_function_lines':len(body.splitlines()),'complete_old_limit_wrapper_lines':len(wrapper.splitlines()),'complete_old_String_formatter_and_placement_unchanged':True,'complete_old_fmt_writer_file_unchanged':True,'author_root_constructor_loop_and_other_production_regions_unchanged':True,'children':packet['children'],'all_new_sources_joined_into_existing_audit':True,'source_registered_new_selectors':8,'source_registered_original_required_selectors':14,'all_old_archives_and_literal_bytes_exact':True,'global_Source36_and_other_existing_packets':'Requires additive composition by integrator after adoption; no prior global proof relabeled current','executable_qualification':False,'physical_gain_observed':False,'reservation_gain_claim':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
print('PASS complete3 production/audit inverses +2 documentation originals, complete old builder/wrapper/placement/String/writer,5 child bindings/joins,8new+14required source registrations. SOURCE ONLY.')
