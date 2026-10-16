#!/usr/bin/env python3
"""Finite complete parent namespaces; source-only, no Rust or benefit claim."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
subjects=json.loads((D/'component-subjects.json').read_text());fronts={};counts={}
for key,subject in subjects.items():
 rows=json.loads((D/('current-to-'+key+'.json')).read_text());front={}
 for row in rows:
  s=row['source'];b=(R/s).read_bytes();assert sha(b)==row['current_sha256'],(key,s,'current');text=b.decode()
  for e in row['edits']:assert text.count(e['new'])==1,(key,s,'unique');text=text.replace(e['new'],e['old'],1)
  old=text.encode();assert sha(old)==row['original_sha256'];assert old==git('show',subject+':'+s)==(R/row['archive']).read_bytes();front[s]=old
 changed=set(git('diff',subject,'--name-only','--diff-filter=M').decode().splitlines());changed={s for s in changed if not s.startswith(str(D.relative_to(R))+'/')};assert changed==set(front),(key,changed^set(front))
 assert not git('diff',subject,'--name-only','--diff-filter=D').strip()
 fronts[key]=front;counts[key]=len(rows)
# A complete Git change census plus the above whole input equality gives each
# predecessor namespace exactly. Old executable verifiers are kept as historical
# algorithms, not rerun incorrectly against a successor's physical namespace.
B=subjects['bcd'];O=subjects['81a'];F=subjects['ff5'];P=subjects['851']
for s in ['src/cleanup_plan/replay.rs','src/cleanup_plan/replay/path_join.rs','src/cleanup_plan/replay/branch_clone/inline_tests.rs','src/cleanup_plan/replay/final_output.rs','src/cleanup_plan/replay/final_output/before.rs','src/cleanup_plan/replay/final_output/tests.rs','src/cleanup_plan/replay/path_join/before_final_edge.rs','src/cleanup_plan/replay/path_join/final_edge_tests.rs']:
 assert (R/s).read_bytes()==git('show',O+':'+s),s
for s in ['src/source_verify/declaration.rs','tests/quickstart_v1/fused_source.rs','src/source_reflows/semantic_receivers.rs','docs/evidence/receiver-source-custody-v1/inverse.json','docs/evidence/receiver-source-custody-v1/README.md']:
 assert (R/s).read_bytes()==git('show',F+':'+s),s
assert (R/'src/source_verify/hints.rs').read_bytes()==git('show',P+':src/source_verify/hints.rs')
for s in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/workspace_graph/type_proof_tests.rs','tests/wasm/scalar_browser_ci_contract.rs','tests/wasm/scalar_browser_ci_contract/historical_builder.rs','tests/wasm/scalar_browser_ci_contract/historical_main2600.rs','tests/wasm/scalar_browser_ci_contract/browser_custody.rs','platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs','src/cleanup_plan/replay/path_array/tests.rs','src/cleanup_plan/replay/path_array/funded_tests.rs','src/workspace_graph/function_alias/before_core/tests.rs','tests/cli_help_surface_v1.rs','crates/semaprax-toolchain/tests/cli_help_surface_v1.rs']:
 assert (R/s).read_bytes()==git('show',B+':'+s),s
# All90 allowlisted runtime fixtures remain the exact Git objects; the complete
# literal inventory/membership are also equal, with no broad parse/rehash.
changed=set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())
assert {s for s in changed if s.startswith('docs/evidence/')}=={'docs/evidence/receiver-source-custody-v1/inverse.json','docs/evidence/receiver-source-custody-v1/README.md'}
helper=(R/'src/source_reflows_merge.rs').read_text();old=git('show',B+':src/source_reflows_merge.rs').decode()
def function(s,n):
 a=s.index('fn '+n+'(');return s[a:s.index('\n}',a)+2]
assert function(helper,'project')==function(old,'project')
assert function(helper,'pin47_input').startswith('fn pin47_input(source: &str, current: &str) -> String {\n    let joined = future_bcd_input(source, current);')
assert 'let joined = future_main8fb_input(source, current);'in function(helper,'main8fb_input')
# Every new compiled source is authenticated and joins each original raw union
# exactly once; production/test authority remains separately current/raw.
owners=json.loads((D/'new-owners.json').read_text());new=git('diff',B,'--name-only','--diff-filter=A','--','src','tests').decode().splitlines();assert {o['source']for o in owners}=={p for p in new if p.endswith('.rs')}
for o in owners:
 b=(R/o['source']).read_bytes();assert sha(b)==o['sha256'];assert len(b.splitlines())==o['lines']<=1500
 for s in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
  assert len(re.findall(r'include_str!\(\s*"[^"]*/'+re.escape(o['source'])+'"', (R/s).read_text()))==1,(s,o['source'])
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_independent_parent_namespaces':counts,'shipping_transfers_exact_reviewed81a':True,'main_declaration_Node_receivers_exact_ff5':True,'hints_correction_exact851':True,'original90_runtime_fixtures_and_pin_successor_controls_unchanged':True,'original_project_body_exact':True,'new_Rust_owners':len(owners),'current_authority':'SEPARATE_REQUIRED_VERIFY_AUTHORITY','old_verifier_reuse':'complete restored input namespace, no new Rust execution','new_runtime_or_pin_PASS_claim':False},indent=2))
