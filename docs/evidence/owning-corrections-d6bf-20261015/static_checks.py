"""Immediate source and complete region projections only; no Rust execution."""
from pathlib import Path
import hashlib,json,subprocess
r=Path.cwd();p=r/'docs/evidence/owning-corrections-d6bf-20261015';basis='d6bf8151942190a956eb57350fa8b8892a8c68e5';sha=lambda b:hashlib.sha256(b).hexdigest()
for row in json.loads((p/'source-inverses.json').read_text())+json.loads((p/'local-region-metadata-inverse.json').read_text())['owners']:
 text=(r/row['source']).read_text();assert sha(text.encode())==row['current_sha256']
 for edit in row['edits']:assert text.count(edit['new'])==1;text=text.replace(edit['new'],edit['before'],1)
 assert text.encode()==(r/row['archive']).read_bytes()==subprocess.check_output(['git','show',basis+':'+row['source']]);assert sha(text.encode())==row['original_sha256']
 if row['source'].endswith('.rs'):assert len((r/row['source']).read_text().splitlines())<=1500
f=r/'docs/evidence/region-entry-receiving-816/exact-source-inverses.json';rows=json.loads(f.read_text());old=json.loads(subprocess.check_output(['git','show',basis+':'+str(f.relative_to(r))]));receipts=json.loads((p/'authenticated-region-checkpoints.json').read_text())
for row,before,receipt in zip(rows,old,receipts):
 assert {k:v for k,v in row.items() if k!='edits'}=={k:v for k,v in before.items() if k!='edits'}
 assert row['edits'][:-1]==before['edits'];extra=row['edits'][-1];assert extra['after']==(r/row['source']).read_text()
 assert extra['before'].encode()==subprocess.check_output(['git','show',receipt['checkpoint_revision']+':'+row['source']]);assert sha(extra['before'].encode())==receipt['checkpoint_sha256']
 text=(r/row['source']).read_text()
 for edit in reversed(row['edits']):assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
 assert text==(f.parent/row['archive']).read_text() and sha(text.encode())==row['sha256']
# Production, full independent drivers/receivers and six-field verdict oracles stay exact.
kept=['src/cleanup_plan/replay/selected_entry.rs','src/cleanup_plan/replay/scalar_replay.rs','src/cleanup_plan/replay/scalar_replay_tree.rs','src/cleanup_plan/replay/lexical_replay.rs','src/cleanup_plan/replay/lexical_replay/before_operands.rs','src/cleanup_plan/replay/lexical_replay/ordinary.rs','src/cleanup_plan/replay/structural_control/blocks.rs','src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs','src/cleanup_plan/replay/structural_control/calls_before_receipt.rs','src/cleanup_plan/replay/structural_control/call_receiving.rs','src/cleanup_plan/replay/expression_facts/receiving/return_operands.rs','src/cleanup_plan/replay/lexical_replay/operand_tests.rs','src/cleanup_plan/replay/semantic_refusal/tests.rs']
for name in kept:assert (r/name).read_bytes()==subprocess.check_output(['git','show',basis+':'+name]),name
# No other history metadata is composed; only the four local missing projections.
changed=subprocess.check_output(['git','diff','--name-only',basis],text=True).splitlines();json_changes=[n for n in changed if n.endswith('.json') and not n.startswith(str(p.relative_to(r)))];assert json_changes==['docs/evidence/region-entry-receiving-816/exact-source-inverses.json'],json_changes
s=(r/'src/cleanup_plan/replay/selected_entry/tests/fact_errors.rs').read_text();assert 'structural_control::validate_blocks_and_edges(' in s and 'structural_control::validate_blocks_and_edges_paid::<expression_facts::errors::EngineError>' in s
assert 'assert_eq!(old.0 + before.0, current.0 + after.0)'in s and 'assert_eq!(old.1 + after.1, current.1 + before.1)'in s
s=(r/'src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs').read_text()
for exact in ['assert_eq!(facts.exits.len(), 1);','let work = 14 + 1 + 1 + 3;','assert_eq!(outcome.is_ok(), allowance == bytes);','assert!(observed.allocated <= used);','assert_eq!(MAX_REPLAY_WORK_UNITS - budget.remaining, expected_work);','assert_eq!(producer.0, 14 + facts.exits.len() + directory.0);','assert_eq!(producer.1, controls() + directory.1);']:assert exact in s,exact
assert 'if false {return 7;}' not in s and 'f.id.as_str() == "main:é"' in s and 'function.cleanup_plan.lexical_control.is_some()'in s
proof={'status':'PASS_SOURCE_ONLY_UNEXECUTED','basis':basis,'immediate_complete_changed_Rust_originals':3,'local_region_metadata_originals':1,'full_region_current_to_immutable_be2aa_to_original_rows':4,'all_old_region_hooks_fields_archives_preserved':True,'all_other_history_metadata_composed':False,'production_and_complete_independent_drivers_receivers_unchanged':kept,'selected_call_controls':'independent full eleven-field layout plus actual complete old/current block owner seams; separate Context seams; absolute driver totals retained','ordinary_832_controls':'independent fourteen-field layout + inspected ExitFacts + actual standalone Directory operation work/bytes; selected path explicitly excluded','return_fixture':'actual lexical while/conditional returns and exact authored stable ID; original exact/short/zero/sticky/Drop assertions retained','cargo':'UNEXECUTED','rust_tests':'UNEXECUTED','clippy':'UNEXECUTED'};(p/'source-only-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
