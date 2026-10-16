"""Finite whole-namespace successor equality; no Cargo/Rust/help execution."""
from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='0e7b370445b1cdad5529b4bcf0652b2dcfa997d8';T='ba5fe75d67d8138d345f618db697c58226e7c895';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,b):
 assert sha(b)==row['current_sha256'],row['source'];s=b.decode()
 for edit in row['edits']:assert s.count(edit['new'])==1,row['source'];s=s.replace(edit['new'],edit['old'],1)
 b=s.encode();assert sha(b)==row['original_sha256']and b==(R/row['archive']).read_bytes();return b
def stage(table,target,view):
 out=dict(view);rows=json.loads(table.read_text())
 for row in rows:
  p=row['source'];out[p]=restore(row,view.get(p,(R/p).read_bytes()));assert out[p]==git('show',target+':'+p)
 return out,len(rows)
v,n=stage(D/'current-to-0e7b.json',B,{})
assert set(v)==set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())
assert not git('diff',B,'--name-only','--diff-filter=D').strip()
# Every pre-existing current input is either unchanged or restored exactly to
# Git0e7b. Therefore the complete predecessor namespace/proof is reused, not a
# weakened rerun against mismatched current inputs.
prior=R/'docs/evidence/b3a-compile-fixture-task722-join-20261016'
for name in ['verify-source.py','verify-authority.py','source-proof.json','authority-source-proof.json']:
 p=prior/name;assert p.read_bytes()==git('show',B+':'+str(p.relative_to(R)))
assert json.loads((prior/'source-proof.json').read_text())['status']=='SOURCE_ONLY_PASS'
for table,target in [('b3a-compile-fixture-task722-join-20261016/current-to-b3a.json','b3a53367d4def76240d14dca25ee414785bed9d6'),('post-main0ab1-singleton-join-20261016/current-to-3177.json','3177d7063ada4fd191994aac0e143d1d0dca029c'),('main696-funded-receiving-joined-20261016/current-to-ff7e.json','ff7e1afd945de67b0e51125168a273e6f93fc73c'),('main696-source-reconciliation-20261016/current-to-364d.json','364d824194cd1df76b5c273aa0fb9cbafb602e29')]:v,c=stage(R/'docs/evidence'/table,target,v)
t,nt=stage(D/'current-to-ba5f.json',T,{})
original_bytes=Path.read_bytes;original_text=Path.read_text
try:
 def data(path):return t.get(str(path.relative_to(R)),original_bytes(path)) if path.is_relative_to(R) else original_bytes(path)
 def text(path,*args,**kwargs):return data(path).decode(kwargs.get('encoding')or'utf-8')
 Path.read_bytes=data;Path.read_text=text
 p=R/'docs/evidence/held-fitting-lexical-comparison-759-20261016/verify-source.py';exec(compile(p.read_text(),str(p),'exec'),{'__file__':str(p),'__name__':'__main__'})
finally:Path.read_bytes=original_bytes;Path.read_text=original_text
assert not git('diff',B,'--name-only','--diff-filter=M','--','docs/evidence/**/*.json').strip()
for p in ['src/cleanup_plan/replay/factored/strategy.rs','src/cleanup_plan/replay/factored/strategy/before_fitting.rs','src/cleanup_plan/replay/factored/strategy/fitting_tests.rs']:assert (R/p).read_bytes()==git('show',T+':'+p)
children=['src/cleanup_plan/replay/factored/strategy/before_fitting.rs','src/cleanup_plan/replay/factored/strategy/fitting_tests.rs']
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 for child in children:assert (R/p).read_text().count('/'+child+'"')==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
proof={'status':'SOURCE_ONLY_PASS','whole_current_to_0e7b_rows':n,'independent759_doc_rows':nt,'predecessor_proof_reused_only_via_complete_existing_namespace_equality':True,'all_prior_JSON_and90_runtime_files_immutable':True,'actual759_production_and_two_children_exact_reviewed_component':True,'original759_verifier_executed_in_authenticated_component_view':True,'all3_raw_joins':True,'Rust_executed':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
