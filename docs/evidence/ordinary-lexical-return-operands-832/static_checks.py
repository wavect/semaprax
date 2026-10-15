"""Whole-source and algorithm seam proof only; no Cargo/compiler/test execution."""
from pathlib import Path
import hashlib,json,subprocess
R=Path.cwd();D=R/'docs/evidence/ordinary-lexical-return-operands-832'
sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
def formatted(text):
 p=subprocess.run(['rustfmt','--emit','stdout','--edition','2024','--config','skip_children=true'],input=text,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 assert p.returncode==0,p.stderr
 return p.stdout
rows=json.loads((D/'exact-source-inverses.json').read_text())
for row in rows:
 source=(R/row['source']).read_text();assert sha(source)==row['current_sha256'],row['source']
 for edit in row['edits']:
  assert source.count(edit['new'])==1,row['source'];source=source.replace(edit['new'],edit['before'],1)
 assert source==row['original_bytes'] and sha(source)==row['original_sha256'],row['source']
 assert len((R/row['source']).read_text().splitlines())<=1500,row['source']
old=(D/'original/src__cleanup_plan__replay__lexical_replay.rs.before.txt').read_text()
assert old==(R/'src/cleanup_plan/replay/lexical_replay/before_operands.rs').read_text()
# Current caller is the full original plus only the exact private module/dispatch
# additions recorded in this packet. Every original line remains byte-exact.
current=(R/'src/cleanup_plan/replay/lexical_replay.rs').read_text()
expected=old.replace('use crate::ast::LexicalExitKind;','''use crate::ast::LexicalExitKind;
mod ordinary;
#[cfg(test)]
#[allow(dead_code, reason = "complete independently compiled original lexical caller")]
mod before_operands;
#[cfg(test)]
mod operand_tests;''',1).replace(') -> Result<(), E> {\n    let plan = &function.cleanup_plan;', ''') -> Result<(), E> {
    if !budget.selected() && budget.pays_allocations() {
        return ordinary::validate_core(program, function, budget, helpers, build_facts);
    }
    let plan = &function.cleanup_plan;''',1)
assert current==expected
# Reconstruct the ordinary driver from the old complete caller with exact,
# enumerated branch/storage seams, preserving every old verification body/order.
s=old[old.index('fn validate_core<'):].replace('fn validate_core<','pub(super) fn validate_core<',1)
a=s.index('    let selected = budget.selected();');b=s.index('\n    let facts = lexical_hir::collect',a)
ordinary=s[a:b];ordinary=ordinary[ordinary.index('    } else {\n')+len('    } else {\n'):];ordinary=ordinary.removesuffix('    };\n').removesuffix('    };')
s=s[:a]+'    let (storage, leaves) = {\n'+ordinary+'    };\n'+s[b:]
a=s.index('    let return_operands = if selected {');b=s.index('    // Lexical functions always replay every path:',a)
s=s[:a]+'''    let return_operands = expression_facts::receiving::ReturnOperands::new::<E>(
        function, &facts, budget,
    )?;
    return_operands.validate_exits(program, function, &facts, &storage, &leaves, budget)?;
    let auth = return_operands.authenticate_certificates(program, function, &owner, &facts, budget)?;
    // Borrowed identity backing and its original owner stay live through auth,
    // CFG/path replay and the final factored comparison, as the old inventory did.
    let _return_operands = return_operands;
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_ordinary_cfg(function, budget)?;
'''+s[b:]
expected='//! Ordinary paid lexical driver; verification order remains independent.\nuse super::*;\n'+s
actual=(R/'src/cleanup_plan/replay/lexical_replay/ordinary.rs').read_text();assert formatted(expected)==formatted(actual)
# The full old paid exits and result-source checks differ only in the guarded,
# fallible full-key membership and their explicit new argument plumbing.
remaining=(D/'references/src__cleanup_plan__replay__expression_facts__receiving__remaining.rs.before.txt').read_text()
a=remaining.index('pub(in crate::cleanup_plan::replay) fn validate_exits(');b=remaining.index('\npub(super) fn validate_finalizer_actions(',a);s=remaining[a:b]
s=s.replace('pub(in crate::cleanup_plan::replay) fn validate_exits(', '#[allow(clippy::too_many_arguments)]\npub(super) fn validate_exits(',1).replace('    return_operands: &BTreeSet<ExpressionId>,','    return_operands: &ReturnOperands<\'_, \'_>,\n    facts: &lexical_hir::LexicalFacts<\'_>,',1)
a=s.index('    if !budget.pays_allocations() {');b=s.index('    let plan =',a);s=s[:a]+s[b:];s=s.replace('leaves, return_operands)?','leaves, return_operands, facts, budget)?')
root=(D/'references/src__cleanup_plan__replay.rs.before.txt').read_text();a=root.index('fn validate_result_source(');b=root.index('\nfn expected_exit_regions(',a)
helper=root[a:b].replace('fn validate_result_source(', '#[allow(clippy::too_many_arguments)]\nfn validate_result_source(',1).replace('    return_operands: &BTreeSet<ExpressionId>,','    return_operands: &ReturnOperands<\'_, \'_>,\n    facts: &lexical_hir::LexicalFacts<\'_>,\n    budget: &mut ReplayBudget,',1).replace('!return_operands.contains(expression)','!return_operands.contains(function, facts, expression, budget)?',1)
expected='//! Complete ordinary paid exit checks, with authenticated paid membership.\nuse super::*;\nuse super::super::remaining::validate_finalizer_actions;\nuse super::super::remaining::selected;\n'+s+'\n'+helper
actual=(R/'src/cleanup_plan/replay/expression_facts/receiving/return_operands/exits.rs').read_text();assert formatted(expected)==formatted(actual)
new=[row['source']for row in rows if row['original_bytes']=='']
for path,prefix in [('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]:
 source=(R/path).read_text()
 for name in new:assert source.count('"'+prefix+name+'"')==1,(path,name)
source=(R/'src/cleanup_plan/replay/expression_facts/receiving/return_operands.rs').read_text()
assert '.clone('not in source and 'BTreeSet::'not in source
assert source.index('reserve_array::<Controls')<source.index('ClassifierLedger::capture()')<source.index('Directory::new(')
proof={'status':'SOURCE_ONLY_NO_COMPILER_OR_TEST_EXECUTION','basis':'d7ec339562b90d655340e315e3ab674ef4d24b12','whole_original_rows_reconstructed':len(rows),'complete_old_caller':'byte_exact_complete_independent_cfg_test_module_compilation_pending','ordinary_driver':'full_old_verification_bodies_and_order_reconstructed','exit_consumer':'full_old_checks_reconstructed_with_exact_paid_membership_seam','complete_source_joins':3,'owned_identity_clones_in_inventory':0,'new_directory_work':'additional_to_original_verification_work','rustfmt_parse_only':True,'cargo':'UNEXECUTED','compiler':'UNEXECUTED','rust_tests':'UNEXECUTED','runtime':'UNEXECUTED','issue_832':'OPEN','issue_781':'OPEN'}
(D/'source-only-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
