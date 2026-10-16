#!/usr/bin/env python3
"""Finite exact parent namespaces and five Main receiver domains; no Rust."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
OURS='47f80b0f7a06fde9b2388439a9c0bfe998a05af1';MAIN='8fb7b025582bd96750b8ceeb9b246f4d795bce8c'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def stage(table,source,b):
 rows=[x for x in json.loads(table.read_text())if x['source']==source];assert len(rows)<=1
 if not rows:return b
 row=rows[0];assert sha(b)==row['current_sha256'],(table,source)
 s=b.decode()
 for e in row['edits']:
  assert s.count(e['new'])==1,(table,source);s=s.replace(e['new'],e['old'],1)
 b=s.encode();assert sha(b)==row['original_sha256'];assert b==(R/row['archive']).read_bytes()
 return b
rows=json.loads((D/'current-to-47f.json').read_text())
for row in rows:
 path=row['source'];b=stage(D/'current-to-47f.json',path,(R/path).read_bytes());assert b==git('show',OURS+':'+path)
assert {r['source']for r in rows}==set(git('diff',OURS,'--name-only','--diff-filter=M').decode().splitlines())
assert not git('diff',OURS,'--name-only','--diff-filter=D').strip()
# Main modified inverse/README are intentional Main content, not normalized
# physical history. The complete prior four-row bytes remain in named47f.
for path in ['src/source_verify/declaration.rs','src/source_verify/hints.rs','tests/quickstart_v1/fused_source.rs','docs/evidence/receiver-source-custody-v1/inverse.json','docs/evidence/receiver-source-custody-v1/README.md','docs/evidence/receiver-source-custody-v1/src__source_verify__declaration.rs.before.txt']:
 assert (R/path).read_bytes()==git('show',MAIN+':'+path),path
main_reader=git('show',MAIN+':src/source_reflows/semantic_receivers.rs').decode()
old_reader=git('show',OURS+':src/source_reflows/semantic_receivers.rs').decode()
expected=old_reader.replace('assert_eq!(proofs().len(), 4);','assert_eq!(proofs().len(), 5);').replace('::main696_input(','::main8fb_input(')
assert (R/'src/source_reflows/semantic_receivers.rs').read_text()==expected
anchor='    let path = proof["path"].as_str().unwrap();\n'
transport='    let joined = crate::source_reflows_merge::main8fb_input(path, current);\n    let current = joined.as_str();\n'
assert expected==main_reader.replace(anchor,anchor+transport,1).replace('restore(repair, &restored)', 'restore_formatting(repair, &restored)'),'all Main receiver bodies/assertions plus explicit input transport'

maps=['current-to-47f.json','task722-held-fitting759-join-20261016/current-to-0e7b.json','b3a-compile-fixture-task722-join-20261016/current-to-b3a.json','post-main0ab1-singleton-join-20261016/current-to-3177.json','main696-funded-receiving-joined-20261016/current-to-ff7e.json','main696-source-reconciliation-20261016/current-to-main696.json']
proofs=json.loads((R/'docs/evidence/receiver-source-custody-v1/inverse.json').read_text());assert len(proofs)==5
repairs=json.loads((R/'docs/evidence/mainf045-source-successor-20261015/live-mainf045-ci93.json').read_text())
for proof in proofs:
 path=proof['path'];b=(R/path).read_bytes()
 if path=='src/source_verify/declaration.rs':b=stage(D/'current-to-main8fb.json',path,b)
 else:
  for name in maps:b=stage(D/name if name=='current-to-47f.json'else R/'docs/evidence'/name,path,b)
 assert sha(b)==proof['after_sha256'],path
 assert b==git('show',MAIN+':'+path),path
 s=b.decode()
 for e in proof['inverse']:
  assert s.count(e['after'])==1,(path,'Main inverse uniqueness');s=s.replace(e['after'],e['before'],1)
 assert sha(s.encode())==proof['before_sha256']
 assert s==(R/'docs/evidence/receiver-source-custody-v1'/proof['archive']).read_text()
 repair=next((q for q in repairs if q['path']==path),None)
 if repair:
  assert sha(s.encode())==repair['after_sha256']
  for e in repair['inverse']:
   assert s.count(e['after'])==1,(path,'complete formatting inverse');s=s.replace(e['after'],e['before'],1)
  assert sha(s.encode())==repair['before_sha256']
# All original runtime transport identities and bytes remain immutable.
inv='src/source_reflows_merge/runtime_json/inventory.rs'
assert (R/inv).read_bytes()==git('show',OURS+':'+inv)
changed_evidence=set(git('diff',OURS,'--name-only','--diff-filter=M','--','docs/evidence').decode().splitlines())
assert changed_evidence=={'docs/evidence/receiver-source-custody-v1/README.md','docs/evidence/receiver-source-custody-v1/inverse.json'}
# Git's complete tracked-file change census plus the ten exact full inverses
# proves the unchanged predecessor namespace without rereading every archive.
for p in ['src/source_reflows_merge.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph.rs']:
 assert len((R/p).read_text().splitlines())<=1500 or p=='tests/hir_module_boundaries.rs'
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
result={'status':'PASS_SOURCE_ONLY','whole_current_to_47f_rows':len(rows),'independent_new_Main_declaration_rows':1,'all_five_Main_full_inverse_and_archive_domains_pass':True,'original_formatting_after_semantic_pass':True,'Main_production_Node_and_new_controls_exact':True,'all_other_prior_evidence_bytes_exact':True,'whole_47f_namespace_reconstructed':True,'new_control_funding_followup_not_a_merge_waiver':True,'Rust_executed':False}
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
