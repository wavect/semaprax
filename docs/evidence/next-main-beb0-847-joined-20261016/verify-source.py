"""Bounded full-byte namespace proof; no Rust execution or runtime verdict."""
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='8bb9f5b7b99e1e71a797460c46180daf716b234b'
def git(*x):return subprocess.check_output(['git',*x],cwd=R)
def sha(b):return hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-8bb.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text());restored={}
for row in rows:
 p=row['source'];new=(R/p).read_bytes();old=git('show',B+':'+p);assert sha(new)==row['current_sha256'] and sha(old)==row['original_sha256'];assert (R/row['archive']).read_bytes()==old
 s=new.decode()
 for e in row['edits']:assert s.count(e['new'])==1;(None);s=s.replace(e['new'],e['old'],1)
 assert s.encode()==old;restored[p]=old
# Exact complete 8bb preexisting source namespace: every modified Rust/docs input
# has a full inverse; unchanged inputs remain identical. No previous checks omitted.
paths=set(git('ls-tree','-r','--name-only',B).decode().splitlines());changed=[]
modified=git('diff',B,'--name-only','--diff-filter=M').decode().splitlines()
for p in modified:
 if p.endswith('.rs') and not p.startswith('docs/evidence/') or p in ['docs/ARCHITECTURE.md','CHANGELOG.md']:
  old=git('show',B+':'+p);assert restored.get(p)==old,p;changed.append(p)
inv=(R/'src/source_reflows_merge/runtime_json/inventory.rs').read_bytes();assert inv==git('show',B+':src/source_reflows_merge/runtime_json/inventory.rs')
old_evidence=[p for p in paths if p.startswith('docs/evidence/')]
assert not [p for p in modified if p.startswith('docs/evidence/')], 'old evidence modified'
# New independent Main namespace and exact graph/Image source cut remain unchanged.
main='faebe78f4904c974c47baf410711e0960f336be1';alias='7b167b98bce5c7dd8042869712af6ca3ca51425d'
owners=json.loads((R/'docs/evidence/main-beb0-independent-controls-20261016/source-owners.json').read_text())['all_three_audit_union_additions'];assert len(owners)==11
for p in owners:assert (R/p).read_bytes()==git('show',main+':'+p)
for row in json.loads((R/'docs/evidence/function-alias-graph-image-parity-847-20261016/source-inverses.json').read_text())['sources']:
 p=row['source'];assert (R/p).read_bytes()==git('show',alias+':'+p)
helper=(R/'src/source_reflows_merge.rs').read_text();before=git('show',B+':src/source_reflows_merge.rs').decode()
prefix='    let next = project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/next-main-beb0-847-joined-20261016/current-to-8bb.json")), source, current);\n    let current = next.as_str();\n'
assert helper.count(prefix)==2 and helper.replace(prefix,'')==before
# Project, all90 loader calls, lazy membership and stage order are consequently exact.
for p in owners:assert len((R/p).read_text().splitlines())<=1500
interop=R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs';assert len(interop.read_text().splitlines())==1500
proof={'status':'SOURCE_ONLY_PASS','base':B,'full_old_input_rows':len(rows),'all_modified_old_source_inputs':changed,'new_main_owners':11,'new_pending_bodies':6,'old_evidence_files_byte_exact':len(old_evidence),'all90_runtime_fixture_specs_unchanged':True,'entire_old_helper_restored':True,'runtime_executed':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
