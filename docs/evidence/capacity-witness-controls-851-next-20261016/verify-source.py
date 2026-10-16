#!/usr/bin/env python3
"""Small component-only before/inverse equality. No Rust or global replay."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest();m=json.loads((D/'source-inverses.json').read_text())
for row in m['rows']:
 old=(D/row['before_archive']).read_bytes();current=(R/row['path']).read_bytes()
 assert sha(old)==row['before_sha256'] and sha(current)==row['current_sha256']
 assert old==subprocess.check_output(['git','show',m['base']+':'+row['path']],cwd=R)
 lines=current.decode().splitlines(keepends=True)
 for edit in reversed(row['inverse_line_edits']):
  a,b=edit['current_start_line']-1,edit['current_end_line']-1
  assert ''.join(lines[a:b])==edit['current'];lines[a:b]=[edit['old']]
 assert ''.join(lines).encode()==old
for row in m['new_sources']:assert sha((R/row['path']).read_bytes())==row['sha256']
old=(D/'hints.rs.before.txt').read_text();current=(R/'src/source_verify/hints.rs').read_text();child=(R/'src/source_verify/hints/before_capacity_witness.rs').read_text()
a=old.index('fn capacity_witness_receiving_bytes()');b=old.index('\n#[cfg(test)]\nmod authoring_repair_tests',a)
assert (D/'helpers.before.rs.txt').read_text()==old[a:b]
parts=[]
for name in ['publication_witness_keeps_exact_repair_and_primary_at_optional_limits','publication_witness_never_replaces_a_sticky_refusal_or_invents_a_repair','publication_composition_pays_all_backing_and_complete_drop']:
 a=old.index('    #[test]\n    fn '+name);b=old.index('\n    }',a)+6;parts.append(old[a:b])
 assert child.count(old[a:b])==1
assert (D/'three-tests.before.rs.txt').read_text()=='\n\n'.join(parts)
expected='//! Complete Main8fb capacity-witness receiver and all three original controls.\nuse super::*;\n\n'+(D/'helpers.before.rs.txt').read_text()+'\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n'+'\n\n'.join(parts)+'\n}\n'
assert child==expected,'entire old receiver plus all original controls'
for name in ['optional_help_pays_exact_backing_and_preserves_primary','short_zero_and_reserved_floor_keep_the_primary_without_refusal','sticky_refusal_and_existing_help_remain_authoritative','reasons_do_not_decorate_unrelated_fields_or_transcript_errors']:
 a=old.index('    #[test]\n    fn '+name);b=old.index('\n    }',a)+6;assert current.count(old[a:b])==1
for name in ['authoring_repair_help','with_authoring_repair_help']:
 a=old.index('fn '+name+'(');b=old.index('\n}',a)+2;assert current.count(old[a:b])==1
assert current.count((D/'helpers.after.rs.txt').read_text())==1
core=current[current.index('pub(super) fn with_capacity_witness'):current.index('\n#[cfg(test)]\nmod before_capacity_witness')]
assert core.index('reserve_active_preserving(capacity_witness_receiving_bytes(), 0)')<core.index('match &diagnostic.help')<core.index('witness.len()')<core.index('reserve_active_preserving(length, 0)')<core.index('String::new()')
assert '.filter('not in core and '.and_then('not in core
assert core.count('.checked_add(')==2 and core.count('composed.push_str(')==3
assert 'composed.push_str(&witness);\n    composed.push_str("\\n\\n");\n    composed.push_str(repair);'in core
assert len(current.splitlines())<=1500 and len(child.splitlines())<=1500
assert not subprocess.check_output(['git','diff',m['base'],'--name-only','--diff-filter=M','--','docs/evidence'],cwd=R).strip()
result={'status':'PASS_SMALL_SOURCE_ONLY','complete_before_inverse_inputs':2,'whole_old_receiver_helpers':2,'original_Main_bodies_unchanged_independently_compiled':3,'shipping_bodies_pending':3,'old_authoring_helpers_and_four_controls_exact':True,'initial_before_receivers_backing_before_String':True,'all_original_JSON_unchanged':True,'global_custody_audits_deferred_to_compositor':True,'Rust_executed':False}
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
