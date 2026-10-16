from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='df2dfeee51ebb0e7c6ba930f604aa2eccf2c5fd2';M='b80d733efe85a85b121d9ab876ff4b9969c4a6df';F='f73b3019cacf5d55d059eeec8d7af0313631ca2d'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
counts={}
for name,rev in [('df2',B),('main',M)]:
 rows=json.loads((D/('current-to-'+name+'.json')).read_text());counts[name]=len(rows)
 for row in rows:
  p=row['source'];b=(R/p).read_bytes();assert sha(b)==row['current_sha256'],p;s=b.decode()
  for e in row['edits']:assert s.count(e['new'])==1,p;s=s.replace(e['new'],e['old'],1)
  assert sha(s.encode())==row['original_sha256'],p
  assert s.encode()==git('show',rev+':'+p)==(R/row['archive']).read_bytes(),p
prior={r['source']for r in json.loads((D/'current-to-df2.json').read_text())}
assert set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())==prior
assert not git('diff',B,'--name-only','--diff-filter=D').strip()
assert not [p for p in prior if p.endswith('.json')], 'old JSON mutated'
helper=(R/'src/source_reflows_merge.rs').read_text();oldhelper=git('show',B+':src/source_reflows_merge.rs').decode()
a=helper.index('fn project(');z=helper.index('fn before_build_fix',a);oa=oldhelper.index('fn project(');oz=oldhelper.index('fn before_build_fix',oa);assert helper[a:z]==oldhelper[oa:oz]
assert 'fn post21_input(source: &str, current: &str) -> String {\n    let joined = main_b80_prior_input(source, current);'in helper
for row in json.loads((D/'new-owners.json').read_text()):assert sha((R/row['source']).read_bytes())==row['sha256']
# Browser component remains byte-exact, preserving both independent local tables.
for p in ['tests/wasm/scalar_browser_ci_contract.rs','tests/wasm/scalar_browser_ci_contract/browser_custody.rs','tests/wasm/scalar_browser_ci_contract/historical_builder.rs','platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs']:
 assert (R/p).read_bytes()==git('show',F+':'+p),p
for n in ['current-to-df2.json','current-to-main.json']:
 for row in json.loads((R/'docs/evidence/main-b80-browser-custody-20261016'/n).read_text()):
  b=(R/row['source']).read_bytes();assert sha(b)==row['current_sha256'];s=b.decode()
  for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
  assert sha(s.encode())==row['original_sha256']
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
adapter=json.loads((D/'main9-format-domain-adapter.json').read_text());text=(R/adapter['source']).read_text();assert sha(text.encode())==adapter['current_sha256']
for edit in adapter['edits']:assert text.count(edit['new'])==1;text=text.replace(edit['new'],edit['old'],1)
assert sha(text.encode())==adapter['original_sha256'];assert text.encode()==git('show',F+':'+adapter['source'])==(R/adapter['archive']).read_bytes()
p={'status':'PASS_SOURCE_ONLY','independent_complete_rows':counts,'whole_df2_namespace_restored':True,'all_prior_JSON_unchanged':True,'browser_component_inputs_exact':True,'project_body_exact':True,'Rust_or_capture_execution':False}
(D/'source-proof.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2))
