from pathlib import Path
import subprocess,json,hashlib,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='83ca0e105e5f4eb6336ed04ff80df6ad0366e99f';M='dd0bae547240ee7bd34d92ac91f24e8806be648b';A='b80d733efe85a85b121d9ab876ff4b9969c4a6df'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(raw,row):
 assert sha(raw)==row['current_sha256'],row['source'];text=raw.decode()
 for e in row['edits']:
  assert text.count(e['new'])==1,row['source'];text=text.replace(e['new'],e['old'],1)
 assert sha(text.encode())==row['original_sha256'],row['source'];return text.encode()
tables={}
for label,rev in [('83ca',B),('dd0',M)]:
 rows=json.loads((D/('current-to-'+label+'.json')).read_text());tables[label]={x['source']:x for x in rows}
 for row in rows:
  p=row['source'];assert restore((R/p).read_bytes(),row)==git('show',rev+':'+p)==(R/row['archive']).read_bytes(),p
changed=set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines());assert changed==set(tables['83ca'])
assert not git('diff',B,'--name-only','--diff-filter=D').strip()
assert not [p for p in changed if p.endswith('.json')]
scope=json.loads((D/'main-owning-scope.json').read_text());assert set(tables['dd0'])==set(scope['all_changed_files_since_base_main'])==set(git('diff','--name-only',A,M).decode().splitlines())
assert len(tables['dd0'])==10 and scope['entire_main_tree_projection']is False
p='src/cleanup_plan/replay/compact_stage/source_custody.rs';s=(R/p).read_text();main=git('show',M+':'+p).decode()
# Only three authenticated input adapters differ from completeMain. All original
# SHA checks, inverse counts, archive equality and both negatives stay verbatim.
s=s.replace('    let joined = crate::source_reflows_merge::main_b80_input("src/cleanup_plan/replay.rs", current);\n    let current = joined.as_str();\n','',1)
s=s.replace('        let current = crate::source_reflows_merge::main_b80_input(path, current);\n        let current = current.as_str();\n','')
s=s.replace('        let path = piece["compiled_path"].as_str().unwrap();\n        let current = match path {','        let current = match piece["compiled_path"].as_str().unwrap() {',1)
assert s==main,'Mainassertions or corpus drift'
assert 'std::fs'not in s and 'read_to_string'not in s
# Authenticate all seven literal compiled inputs through the exact named chain,
# then execute the original seven complete inverses against immutable archives.
helper_tables=[json.loads((D/'current-to-83ca.json').read_text()),json.loads((R/'docs/evidence/session-reference-instance-joined-20261016/current-to-17b.json').read_text()),json.loads((R/'docs/evidence/main-b80-complete-source-join-20261016/current-to-main.json').read_text())]
def main_input(path):
 raw=(R/path).read_bytes()
 compiled=[x for x in json.loads((D/'compiled-before-to-main.json').read_text())if x['source']==path]
 if compiled:
  row=compiled[0];raw=restore(raw,row);assert raw==git('show',A+':'+path)==(R/row['archive']).read_bytes();return raw.decode()
 for rows in helper_tables:
  hits=[x for x in rows if x['source']==path];assert len(hits)<=1
  if hits:raw=restore(raw,hits[0])
 return raw.decode()
packet=R/'docs/evidence/compact-staged-observations-v1';records=json.loads((packet/'inverse.json').read_text())
files=re.findall(r'\(\s*"([^"]+)",\s*source!\("([^"]+)"\),\s*"([^"]+)",\s*source!\("([^"]+)"\),\s*\)',main);assert len(files)==len(records)==7
for row,(path,input_path,archive,archive_path)in zip(records,files):
 assert path==input_path==row['path']and archive==row['archive']and archive_path=='docs/evidence/compact-staged-observations-v1/'+archive
 text=main_input(path);assert sha(text.encode())==row['after_sha256'];restored=text
 for e in row['inverse']:
  assert restored.count(e['after'])==1;restored=restored.replace(e['after'],e['before'],1)
 assert sha(restored.encode())==row['before_sha256']and restored.encode()==(R/archive_path).read_bytes()
pieces=json.loads((packet/'original-pieces.json').read_text());assert len(pieces)==7
for piece in pieces:
 old=piece['body'];compiled=piece['compiled_body'];current=main_input(piece['compiled_path'])
 assert sha(old.encode())==piece['before_sha256']and current.count(compiled)==1
 for e in piece['inverse']:
  assert compiled.count(e['after'])==1;compiled=compiled.replace(e['after'],e['before'],1)
 assert compiled==old
# Parent-authored actualpin front preserves every complete83ca input.
pin=R/'docs/evidence/actual83ca-pin-successor-20261016'
for row in json.loads((pin/'browser-current-to-83ca.json').read_text()):
 assert restore((R/row['source']).read_bytes(),row)==git('show',B+':'+row['source'])
plan=json.loads((pin/'managed-plan/readonly-plan.json').read_text());assert plan['capture_head']==B and len(plan['edits'])==11
for path in {e['path']for e in plan['edits']}:
 candidate=git('show',B+':'+path).decode()
 for e in [x for x in plan['edits']if x['path']==path]:
  assert candidate.count(e['before'])==1;candidate=candidate.replace(e['before'],e['after'],1)
 actual=(R/path).read_text()
 if path=='tests/workspace/semantic_graph.rs':actual=actual.replace('main-dd0-compiled-custody-join-20261016/projection.json','session-reference-instance-joined-20261016/projection.json')
 assert actual==candidate,path
historical='tests/wasm/scalar_browser_ci_contract/historical_builder.rs'
assert (R/historical).read_bytes().startswith(git('show',B+':'+historical))
for path in ['tests/wasm/scalar_browser_ci_contract/historical_main_c451.rs','tests/wasm/scalar_browser_ci_contract/before_c451.rs','tests/wasm/scalar_browser_ci_contract/main_c451_before_2600.rs']:
 assert (R/path).read_bytes()==git('show',B+':'+path)
# Independently replay BOTH complete actual browser wires to Mainc451 and47f;
# only their actualbill and their selfdigest differ. No dualamount acceptance.
for stem in ['baseline','renamed']:
 raw=(pin/'actual-browser'/('calculator-'+stem+'.json')).read_text().removesuffix('\n');j=json.loads(raw);assert j['budget']['used_builder_bytes']==3442219
 for previous in [R/'docs/evidence/browser-graph-pins-c451086ce'/('current-hosted-calculator-'+stem+'.json'),R/'docs/evidence/actual47f-pin-fixture-successor-20261016/actual-browser'/('calculator-'+stem+'.json')]:
  old=previous.read_text().removesuffix('\n');before=json.loads(old)
  field='"used_builder_bytes":3442219';assert raw.count(field)==1
  text=raw.replace(field,'"used_builder_bytes":'+str(before['budget']['used_builder_bytes']),1)
  field='"graph_digest":'+json.dumps(j['graph_digest']);assert text.count(field)==1
  assert text.replace(field,'"graph_digest":'+json.dumps(before['graph_digest']),1)==old
# Concrete complete production equivalence: every changed original path is an
# exact enumerated test input, Changelog, or a closedcfgtest source witness.
src={p for p in changed if p.startswith('src/')};assert src=={'src/source_reflows_merge.rs','src/cleanup_plan/replay/compact_stage/source_custody.rs','src/workspace_graph/type_proof_tests.rs'}
for p in src-{'src/workspace_graph/type_proof_tests.rs'}:
 m=json.loads((D/'hir.json').read_text());assert m['classes'][p]['role']=='test-only'
# The pin-only sourceproof owner is outside the HIR/projection raw inventories;
# bind its exact unchanged closed registration, without widening either policy.
root=(R/'src/workspace_graph.rs').read_text();assert root==git('show',B+':src/workspace_graph.rs').decode()
assert '#[cfg(test)]\n#[path = "workspace_graph/type_proof_tests.rs"]\nmod type_proof_tests;'in root
interop='crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs'
platform='platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'
assert all(p=='CHANGELOG.md'or p.startswith('tests/')or p in src or p in [interop,platform]for p in changed)
# Exact existing closed include chain for the third audit; no production exemption.
for p,registration in [
 ('crates/semaprax-native-rust-interop-builder/src/implementation.rs','#[cfg(test)]\n#[path = "implementation/tests.rs"]\nmod tests;'),
 ('crates/semaprax-native-rust-interop-builder/src/implementation/tests.rs','#[path = "tests/ledger_capacity.rs"]\nmod ledger_capacity;'),
 ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs','include!("ledger_capacity/source_inventory.rs");')]:
 raw=(R/p).read_bytes();assert raw==git('show',B+':'+p)and registration in raw.decode()
owners=json.loads((D/'new-owners.json').read_text());new_sources={p for p in git('diff',B,'--name-only','--diff-filter=A').decode().splitlines()if p.endswith('.rs')and p.split('/')[0]in ['src','tests','crates']}
assert new_sources=={x['source']for x in owners}=={'tests/wasm/scalar_browser_ci_contract/actual_83ca.rs'}
for row in owners:
 assert sha((R/row['source']).read_bytes())==row['sha256']and row['lines']<=1500
 assert json.loads((D/'hir.json').read_text())['classes'][row['source']]['role']=='test-only'
assert all(p.startswith('docs/evidence/')or p in new_sources for p in git('diff',B,'--name-only','--diff-filter=A').decode().splitlines())
assert not git('diff',B,'--name-only','--','Cargo.toml','Cargo.lock','src/main.rs','build.rs','std').strip()
assert git('diff',B,'--name-only','--','crates').decode().splitlines()==[interop]
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 before=re.findall(r'include_str!\(\s*"([^"]+)"',git('show',B+':'+path).decode());after=re.findall(r'include_str!\(\s*"([^"]+)"',(R/path).read_text());after=[x for x in after if not x.endswith('scalar_browser_ci_contract/actual_83ca.rs')];after=[x.replace('main-dd0-compiled-custody-join-20261016/','session-reference-instance-joined-20261016/')for x in after];assert before==after
helper=(R/'src/source_reflows_merge.rs').read_text()
for f in ['main_b80_input','main_b80_prior_input','reference781_input']:
 assert ('fn '+f+'(source: &str, current: &str) -> String {\n    let merged = main_dd0_previous(source, current);')in helper
assert 'main-dd0-compiled-custody-join-20261016/current-to-dd0.json'not in helper
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
proof={'status':'SOURCE_ONLY_PASS_NO_RUST_EXECUTION','complete_current_to83ca_rows':len(tables['83ca']),'scoped_newMain10_inputs_including_identity_rows':len(tables['dd0']),'scope_not_entireMain_tree':True,'all7original_inverses_and7compiled_pieces_pass':True,'independent_complete_Main_compiled_file_views':2,'Mainbody_exact_except3authenticated_input_adapters':True,'oldJSON_and_prior_proofs_unchanged':True,'complete_production_source_equivalence_to83ca':True,'actual11managed_and2browser_pin_literals_verified':True,'newbrowser_module_preserves_both_complete_prior_wires':True,'consumer_or_Main_tests_executed':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
