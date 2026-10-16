#!/usr/bin/env python3
"""Complete successor input equality and precise operation proof; no Rust."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='36f47201b5a87054a846afbfe06c86e9876bff1a';OLD='docs/evidence/future-main-output-851-joined-20261016/'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
rows=json.loads((D/'current-to-36f.json').read_text());assert len(rows)==7
for row in rows:
 p=row['source'];b=(R/p).read_bytes();assert sha(b)==row['current_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 b=s.encode();assert sha(b)==row['original_sha256'];assert b==git('show',B+':'+p)==(R/row['archive']).read_bytes()
assert set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())=={r['source']for r in rows}
assert not git('diff',B,'--name-only','--diff-filter=D').strip()
# Thus every pre-existing input in the committed36f namespace is complete and
# exact before its four independent parent views. Reuse its actual static proof
# only under this whole namespace equality, never as new runtime qualification.
oldproof=json.loads(git('show',B+':'+OLD+'source-proof.json'));assert oldproof['status']=='PASS_SOURCE_ONLY'
assert oldproof['whole_independent_parent_namespaces']=={'bcd':16,'ff5':22,'81a':23,'851':21}
for p in git('ls-tree','-r','--name-only',B,'--',OLD).decode().splitlines():
 assert (R/p).read_bytes()==git('show',B+':'+p),p
for p in ['src/workspace_graph/function_alias/before_core/tests.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p),p
for p,subject in [('src/workspace_graph.rs','d781576580a03d8e8ed9dccb96054a2be3c07f35'),('src/cleanup_plan/replay/path_array/funded_tests.rs','16b7c8a1fedd546e909141446d02d6c9155f9cf5')]:
 assert (R/p).read_bytes()==git('show',subject+':'+p),p
helper=(R/'src/source_reflows_merge.rs').read_text()
for fn in ['future_bcd_input','future_main8fb_input']:
 assert 'pub(crate) fn '+fn+'(source: &str, current: &str) -> String {\n    let joined = post21_input(source, current);'in helper
# Existing original project body, receivers, source guards and all90 fixture
# identities stay exact via the complete36f input reconstruction above.
outputs=[]
for p in ['docs/evidence/path-array-operation-observer-21-next-20261016/verify-source.py','docs/evidence/first-uncached-alias-route-847-20261016/verify.py']:
 outputs.append({'verifier':p,'sha256':sha((R/p).read_bytes()),'stdout':subprocess.check_output(['python3',str(R/p)],cwd=R,text=True).strip()})
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
proof={'status':'PASS_SOURCE_ONLY','whole_current_to36f_rows':7,'entire_committed36f_input_namespace_restored':True,'reused_prior_static_proof_only_by_exact_namespace':oldproof,'two_exact_component_static_replays':outputs,'original90_and_all_prior_JSON_unchanged':True,'source_authority':'SEPARATE_CURRENT_VERIFY_AUTHORITY_REQUIRED','actual_prior21':'19PASS/2FAIL; all15pinsPASS; history only, not current new-source PASS','next_scope':'49 affected plus original15 pin consumers,64unique; capture-first producer3 then final4commands','Rust_or_capture_execution':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
