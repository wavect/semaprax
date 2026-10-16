"""Finite source equality and exact original proof reuse; no Rust/help execution."""
from pathlib import Path
import hashlib,json,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='b3a53367d4def76240d14dca25ee414785bed9d6';T='1fea2e23c8f5d7a61d75d49a54e5e27f82cf02f2';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,b):
 assert sha(b)==row['current_sha256'],row['source'];s=b.decode()
 for edit in row['edits']:
  assert s.count(edit['new'])==1,row['source'];s=s.replace(edit['new'],edit['old'],1)
 b=s.encode();assert sha(b)==row['original_sha256'],row['source'];assert b==(R/row['archive']).read_bytes();return b
def stage(path,target,view):
 out=dict(view);rows=json.loads(path.read_text())
 for row in rows:
  p=row['source'];out[p]=restore(row,view.get(p,(R/p).read_bytes()));assert out[p]==git('show',target+':'+p)
 return out,len(rows)
v,n=stage(D/'current-to-b3a.json',B,{})
assert {x['source']for x in json.loads((D/'current-to-b3a.json').read_text())}==set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())
for table,target in [('post-main0ab1-singleton-join-20261016/current-to-3177.json','3177d7063ada4fd191994aac0e143d1d0dca029c'),('main696-funded-receiving-joined-20261016/current-to-ff7e.json','ff7e1afd945de67b0e51125168a273e6f93fc73c'),('main696-source-reconciliation-20261016/current-to-364d.json','364d824194cd1df76b5c273aa0fb9cbafb602e29')]:v,count=stage(R/'docs/evidence'/table,target,v)
# Both actual compiler corrections preserve the complete remaining fixture bytes.
p='src/cleanup_plan/replay/path_array/funded_tests.rs';old=git('show',B+':'+p).decode();assert old.count('drop((target, work, budget));')==1;assert (R/p).read_text()==old.replace('drop((target, work, budget));','drop((target, work));\n                drop(budget);',1)
p='src/cleanup_plan/replay/path_array/before.rs';old=git('show',B+':'+p).decode();assert (R/p).read_text()==old.replace('use super::*;\n','',1)
# Independent component view preserves the original full module, including the
# reviewed 84-byte product-copy deletion, without rewriting its original packet.
t,n722=stage(D/'current-to-1fea.json',T,{})
p='src/cli/help/authoring/tasks.rs';sentence=' Qualification: existing starter definitions; help asserts no new executable result.';assert len(sentence.encode())==84;assert (R/p).read_bytes()==git('show',T+':'+p).replace(sentence.encode(),b'',1)
original=Path.read_bytes
try:
 def read(path):return t.get(str(path.relative_to(R)),original(path)) if path.is_relative_to(R) else original(path)
 Path.read_bytes=read
 p=R/'docs/evidence/task-guidance-v6-722-20261016/verify-source.py';exec(compile(p.read_text(),str(p),'exec'),{'__file__':str(p),'__name__':'__main__'})
finally:Path.read_bytes=original
assert not git('diff',B,'--name-only','--diff-filter=M','--','docs/evidence/**/*.json').strip()
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/cleanup_plan/replay.rs','src/cleanup_plan/replay/path_array.rs','src/cleanup_plan/replay/factored/canonical_shared.rs','src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs','src/cleanup_plan/replay/path_join.rs']:assert (R/p).read_bytes()==git('show',B+':'+p),p
helper=(R/'src/source_reflows_merge.rs').read_text();old=git('show',B+':src/source_reflows_merge.rs').decode();assert helper[helper.index('fn project('):helper.index('// Exact post-build')]==old[old.index('fn project('):old.index('// Exact post-build')]
new=['src/cli/help/authoring/tasks.rs','src/cli/help/authoring/tasks/tests.rs','src/cli/help/authoring/tasks/derive.rs','tests/cli_help_surface_v1/task_pages.rs']
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 for child in new:assert (R/p).read_text().count('/'+child+'"')==1,(p,child)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
caps={}
for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line and not line.startswith('#'):p,ncap=line.split();caps[p]=int(ncap)
for p in git('diff',B,'--name-only','--','src','tests','crates').decode().splitlines():
 if p.endswith('.rs')and (R/p).is_file():assert len((R/p).read_text().splitlines())<=caps.get(p,1500),p
proof={'status':'SOURCE_ONLY_PASS','current_to_b3a_rows':n,'independent722_rows':n722,'mechanical_compiler_fixes_preserve_full_other_bytes':True,'original722_proof_executed_against_exact_restored_component_namespace':True,'capsule_product_copy_removed_bytes':84,'four_new_Rust_owners_all3_raw_joins':True,'prior_production_cuts_and_90_runtime_fixtures_unchanged':True,'all_old_JSON_immutable':True,'project_body_exact':True,'Rust_executed':False,'failed_b3a_status':'actual first unit101, zero bodies, three commands unrun; preserved in parent published archive'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
