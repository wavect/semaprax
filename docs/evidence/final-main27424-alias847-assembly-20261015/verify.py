#!/usr/bin/env python3
"""Bounded final two-branch source replay. No Rust/test/toolchain execution."""
from pathlib import Path
import hashlib,json,subprocess,contextlib,io
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;h=lambda b:hashlib.sha256(b).hexdigest();g=lambda *a:subprocess.check_output(['git',*a],cwd=R)
refs={'main':'07e72e00d108fcb5bb6ce9f28ea376610431769a','alias':'29c549b6114e5c5238447f169135aa8e82ce4a44'};views={};ns=json.loads((D/'named-domains.json').read_text())
for stage,rev in refs.items():
 rows=json.loads((D/f'final-to-{stage}-checkpoint.json').read_text());views[stage]={}
 for row in rows:
  p=R/row['source'];raw=p.read_bytes();assert h(raw)==row['current_sha256'];text=raw.decode()
  for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
  assert h(text.encode())==row['original_sha256'];assert text.encode()==(R/row['archive']).read_bytes()==g('show',rev+':'+row['source']);views[stage][p.resolve()]=text.encode()
def replay(stage,relative):
 aliases=views[stage];excluded={(R/p).resolve() for p in ns[stage]['introduced_paths']};script=R/relative;code=script.read_text();captured=[]
 rb,rt,wt,co,glob,exists=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob,Path.exists
 def output(args,*a,**k):
  if isinstance(args,list) and args[:2]==['git','diff']:
   i=args.index('--') if '--' in args else len(args);revs=[v for v in args[2:i] if not v.startswith('-')]
   if len(revs)==1:args=args[:i]+[refs[stage]]+args[i:]
  return co(args,*a,**k)
 def rglob(p,*a,**k):
  for child in glob(p,*a,**k):
   if child.resolve() not in excluded:yield child
 def write(p,text,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(text));return len(text)
 try:
  Path.read_bytes=lambda p,*a,**k:aliases[p.resolve()] if p.resolve() in aliases else rb(p,*a,**k)
  Path.read_text=lambda p,*a,**k:aliases[p.resolve()].decode() if p.resolve() in aliases else rt(p,*a,**k)
  Path.write_text=write;subprocess.check_output=output;Path.rglob=rglob;Path.exists=lambda p:False if p.resolve() in excluded else exists(p)
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob,Path.exists=rb,rt,wt,co,glob,exists
 return captured[-1] if captured else json.loads(out.getvalue())
p_main=replay('main','docs/evidence/main27424-source-integration-20261015/verify.py');p_alias=replay('alias','docs/evidence/retained-alias-compact-joined-20261015/verify.py')
# Current algorithms are exact independent reviewed checkpoints; only cfg(test) shared plumbing differs.
main_files=['src/cleanup_plan/replay/structural_control/calls.rs','src/cleanup_plan/replay/structural_control/calls/cursor.rs','src/cleanup_plan/replay/structural_control/calls/cursor/tests.rs','src/cleanup_plan/replay/structural_control/calls/cursor_before.rs','src/cleanup_plan/replay/structural_control/calls/cursor_before_selected.rs','src/ast/expression_statement.rs','src/ast/expression_statement/tests.rs','src/cleanup_plan/replay/physical_budget.rs']
alias_files=['src/ast/program_source.rs','src/ast/program_source/decl_slices.rs','src/hir/identity_resolution.rs','src/workspace_graph.rs','src/workspace_graph/checked_retention.rs','src/workspace_graph/constructor_failure/typed.rs','src/workspace_graph/expected_projection/borrowed_imports/typed.rs','src/workspace_graph/expected_projection/borrowed_leaf.rs']
for stage,files in [('main',main_files),('alias',alias_files)]:
 for path in files:assert (R/path).read_bytes()==g('show',refs[stage]+':'+path),path
# Each full protected branch packet remains immutable.
kept=0
for rev,packet in [(refs['main'],'main27424-source-integration-20261015'),(refs['alias'],'retained-alias-compact-joined-20261015'),(refs['alias'],'retained-function-alias-847')]:
 for name in g('ls-tree','-r','--name-only',rev,'--','docs/evidence/'+packet).decode().splitlines():assert (R/name).read_bytes()==g('show',rev+':'+name),name;kept+=1
children=[r['source'] for r in json.loads((R/'docs/evidence/retained-function-alias-847/new-owners.json').read_text())]+['src/cleanup_plan/replay/structural_control/calls/'+p for p in ['cursor.rs','cursor/tests.rs','cursor_before.rs','cursor_before_selected.rs']]
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/path).read_text()
 for child in children:
  revision = refs['alias'] if child in {r['source'] for r in json.loads((R/'docs/evidence/retained-function-alias-847/new-owners.json').read_text())} else refs['main']
  count = text.count(child[4:]+'"')
  expected = g('show',revision+':'+path).decode().count(child[4:]+'"')
  assert count >= 1 and count == expected,(path,child,count,expected)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
result={'status':'SOURCE_ONLY_FINAL_JOIN_NOT_QUALIFIED','independent_final_fronts':{k:len(v) for k,v in views.items()},'prior_MAIN_full_proof':p_main,'prior_alias_full_proof':p_alias,'reviewed_production_and_test_algorithms_byteexact':True,'protected_branch_packet_files_byteexact':kept,'new_children_each_three_audits':len(children),'all_owner_occurrence_counts_match_reviewed_checkpoint':True,'Rust_Cargo_Node_tests_Clippy_executed':False,'new_pin_or_app_success_claim':False}
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if not k.startswith('prior_')},indent=2))
