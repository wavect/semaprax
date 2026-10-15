"""Source-only replay of current owning predicates; never executes Rust."""
from pathlib import Path
import subprocess,re,json
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
s=(R/'tests/workspace/semantic_graph/source_inventory.rs').read_text().split('macro_rules! semantic_graph_private_layout_source_inventory')[1].split('\nmacro_rules!')[0]
joined=[];allowed='src/workspace_graph/expected_projection/private_layout/function_runtime.rs'
for ref in re.findall(r'include_str!\(\s*"([^"]+)"',s):
 p=R/ref[ref.index('src/'):] if 'src/'in ref else R/'tests/workspace'/ref
 if p.is_file():
  text=p.read_text();name=str(p.relative_to(R));assert 'use super::*;'not in text,name
  if name!=allowed:
   for word in ['workspace::','std::fs','std::process','snapshot(','build_authenticated','render_graph_json','write_file','create_dir','remove_file','ACTIVE']:assert word not in text,(name,word)
  joined.append(name)
limits={}
for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
 if not line.strip()or line.startswith('#'):continue
 a=line.split('\t');limits[a[0]]=int(a[1])
# Exact roots in tests/module_size.rs::tracked_sources, including each crate.
roots=[R/'src',R/'tests']
for crate in (R/'crates').iterdir():roots.extend([crate/'src',crate/'tests'])
counts={str(p.relative_to(R)):len(p.read_text().splitlines())for root in roots if root.is_dir()for p in root.rglob('*.rs')}
violations=[(name,count,limits.get(name,1500))for name,count in counts.items()if count>limits.get(name,1500)]
assert not violations,violations
for name in limits:assert counts[name]>1500,(name,'obsolete size budget entry')
B=R/'docs/evidence/selected-session-static-refusal-781/original';atom=(B/'src__cleanup_plan__replay__semantic_refusal.rs.before.txt').read_text();error=(B/'errors.rs.before.txt').read_text();mirror=(R/'src/cleanup_plan/replay/selected_entry/tests/session_errors.rs').read_text()
def body(s,m):return s.split(m,1)[1].split('}',1)[0]
def compact(s):return ''.join(s.split())
assert compact(body(atom,'enum Atom {'))==compact(body(mirror,'enum Atom {'))
assert compact(body(error,'enum EngineError {'))==compact(body(mirror,'enum EngineError {')).replace('expression_facts::errors::','')
assert compact(body(atom,'source: ExpressionId,'))==compact(body(mirror,'source: ExpressionId,'))
kept=['tests/workspace/semantic_graph.rs','docs/COMPLETION-MATRIX.md']
for name in kept:assert (R/name).read_bytes()==subprocess.check_output(['git','show','4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc:'+name],cwd=R)
(D/'static-integration-checks.json').write_text(json.dumps({'status':'SOURCE_ONLY_NO_COMPILER_OR_RUNTIME','private_layout_sources':len(joined),'all_forbidden_literals_preserved_without_new_exemption':True,'module_size_scope':'exact roots of tests/module_size.rs::tracked_sources','measured_rust_sources':len(counts),'module_size_violations':violations,'all_three_session_mirror_source_assertions_equal':True,'initial_public_pin_owner_and_completion_matrix_unchanged':kept,'new_guard_exemptions':0,'initial_root':'4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc'},indent=2)+'\n')
print('source-only purity',len(joined),'sources; exact owning line-budget roots',len(counts),'; mirror3 +initial pin/matrix bytes verified')

# All newly split typed projection producers remain in the owning audited union.
inventory=(R/'tests/workspace/semantic_graph/source_inventory.rs').read_text()
projection=inventory.split('macro_rules! semantic_graph_projection_root_source_inventory {',1)[1]
children=['borrowed_imports/typed.rs','defaults/typed.rs','defaults/typed_fields.rs','import_stub/typed.rs']
for child in children:
 name='src/workspace_graph/expected_projection/'+child
 assert projection.count('include_str!("../../'+name+'")')==1,name
 text=(R/name).read_text()
 for word in ['use super::*;','workspace::','std::fs','std::process','snapshot(','build_authenticated','render_graph_json','write_file','create_dir','remove_file','ACTIVE']:
  assert word not in text,(name,word)
p=D/'static-integration-checks.json';proof=json.loads(p.read_text());proof['typed825_projection_children_joined_and_pure']=children;p.write_text(json.dumps(proof,indent=2)+'\n')
print('typed825 projection purity joins:',len(children),'complete children; no new exclusions')
