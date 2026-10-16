from pathlib import Path
import subprocess,json,hashlib,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='17b12c909916ce8671b43f208b59f3432144709a'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def definition(s,needle):
 start=s.index(needle);a=s.index('{',start);depth=1;i=a+1
 while depth:depth+=(s[i]=='{')-(s[i]=='}');i+=1
 return s[start:i],s[a:i]
def tokens(s):
 pattern=r'"(?:\\.|[^"\\])*"|//[^\n]*|/\*.*?\*/|[A-Za-z_]\w*|\d+|[^\s]'
 return [x for x in re.findall(pattern,s,re.S)if not x.startswith('//')and not x.startswith('/*')]
rows=json.loads((D/'source-inverses.json').read_text())
for row in rows:
 p=row['source'];b=(R/p).read_bytes();assert sha(b)==row['current_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'];assert s.encode()==git('show',B+':'+p)==(R/row['archive']).read_bytes()
for name in ['lexical_replay.rs','scalar_replay.rs']:
 p='src/cleanup_plan/replay/'+name;old=git('show',B+':'+p).decode();current=(R/p).read_text()
 generic,body=definition(old,'fn validate_core<');assert definition(current,'fn validate_core<')[0]==generic
 wrapper=definition(current,'pub(super) fn validate_paid_before_local_receivers(')[0].replace('fn validate_paid_before_local_receivers(','fn validate_paid(',1)
 assert tokens(wrapper)==tokens(definition(old,'pub(super) fn validate_paid(')[0])
 expected=re.sub(r'\bE\b','expression_facts::errors::EngineError',body);expected=re.sub(r'\bbuild_facts\b','expression_facts::paid_arena',expected);expected=re.sub(r'\bhelpers\b','None',expected)
 expected=expected.replace('selected_profile::validate(','selected_profile::paid::validate(').replace('selected_owner::owner(','selected_owner::paid::owner(')
 paid,_=definition(current,'pub(super) fn validate_paid(');header=paid[:paid.index('{')]
 formatted=subprocess.check_output(['rustfmt','--edition','2024','--config','skip_children=true','--emit','stdout'],input=(header+expected).encode()).decode()
 assert definition(current,'pub(super) fn validate_paid(')[1]==definition(formatted,'pub(super) fn validate_paid(')[1],name
for p in ['src/cleanup_plan/replay/selected_entry/paid.rs','src/cleanup_plan/replay/selected_profile/paid.rs','src/cleanup_plan/replay/selected_owner/paid.rs','src/cleanup_plan/replay/physical_budget.rs','src/workspace_report.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p),p
entry=(R/'src/cleanup_plan/replay/selected_entry.rs').read_text()
for name,callee in [('validate_paid_original','lexical_replay'),('validate_scalar_paid_original','scalar_replay')]:
 whole,body=definition(entry,'pub(crate) fn '+name+'(')
 before=definition(git('show',B+':src/cleanup_plan/replay/selected_entry.rs').decode(),'pub(crate) fn '+name+'(')[0]
 restored=whole.replace(callee+'::validate_paid_before_local_receivers(',callee+'::validate_paid(')
 assert tokens(restored)==tokens(before)
assert set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())=={r['source']for r in rows}
assert not [r for r in rows if r['source'].endswith('.json')]
child='src/cleanup_plan/replay/selected_entry/tests/driver_local_receivers.rs';owner=json.loads((D/'new-owner.json').read_text());assert owner['source']==child and sha((R/child).read_bytes())==owner['sha256']
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 assert (R/p).read_text().count('selected_entry/tests/driver_local_receivers.rs')==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
proof={'status':'SOURCE_ONLY_PASS_NO_RUST_EXECUTION','whole_before_inverse_rows':len(rows),'both_complete_generic_legacy_drivers_byteexact':True,'both_saved_paid_wrappers_exact':True,'specialized_paid_bodies_only_declared_profile_owner_twins_and_removed_callback_forwarding':True,'report_initial_refusal_and_existing_paid_twins_byteexact':True,'new_test_bodies':2,'old_JSON_unchanged':True,'all3_raw_joins':True,'authority_manifest':'FINAL_COMPOSITION_MUST_REGENERATE','owning_runtime':'NOT_EXECUTED','whole781':'OPEN_remaining_Diagnostic_producers_explicit'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
