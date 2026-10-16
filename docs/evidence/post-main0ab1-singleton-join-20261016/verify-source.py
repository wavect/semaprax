"""Finite joined source custody, not a Rust gate or behavior measurement."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='3177d7063ada4fd191994aac0e143d1d0dca029c';F='ff7e1afd945de67b0e51125168a273e6f93fc73c';C='364d824194cd1df76b5c273aa0fb9cbafb602e29';M='6966765c965c8a5ad85ae3c2827653599826fb88';N='0ab1bb6ccb854b1f64e5cb541878c21e5c5ea0cd';S='edacb0b7902889957302f232100bcde8b7d9ee1f';P='d201b9cdab9d45a7b0b5e3a401316408c401507b';U='3ae2fbb5710b9e85aef2d2a91d42a7a782216d76'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,b):
 assert sha(b)==row['current_sha256'],row['source'];s=b.decode()
 for e in row['edits']:
  assert s.count(e['new'])==1,row['source'];s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'],row['source'];return s.encode()
def stage(path,target,inputs):
 rows=json.loads(path.read_text());out=dict(inputs)
 for row in rows:
  p=row['source'];b=restore(row,inputs.get(p,(R/p).read_bytes()));assert b==git('show',target+':'+p);a=row.get('archive',row.get('original_archive'));assert a and b==(R/a).read_bytes();out[p]=b
 return out,len(rows)
v,count=stage(D/'current-to-3177.json',B,{})
assert set(json.loads((D/'current-to-3177.json').read_text())[i]['source']for i in range(count))==set(git('diff',B,'--name-only','--diff-filter=M').decode().splitlines())
vff,nff=stage(R/'docs/evidence/main696-funded-receiving-joined-20261016/current-to-ff7e.json',F,v)
cut,ncut=stage(R/'docs/evidence/main696-source-reconciliation-20261016/current-to-364d.json',C,vff)
main,nmain=stage(R/'docs/evidence/main696-source-reconciliation-20261016/current-to-main696.json',M,vff)
for row in json.loads((R/'docs/evidence/receiver-source-custody-v1/inverse.json').read_text()):
 p=row['path'];b=main.get(p,(R/p).read_bytes());assert b==git('show',M+':'+p) and sha(b)==row['after_sha256'];s=b.decode()
 for e in row['inverse']:
  assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
 assert sha(s.encode())==row['before_sha256'] and s.encode()==(R/'docs/evidence/receiver-source-custody-v1'/row['archive']).read_bytes()
 formatting=next((x for x in json.loads((R/'docs/evidence/mainf045-source-successor-20261015/live-mainf045-ci93.json').read_text())if x['path']==p),None)
 if formatting:
  assert sha(s.encode())==formatting['after_sha256']
  for e in formatting['inverse']:
   assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
  assert sha(s.encode())==formatting['before_sha256']
# Shipping/current owning files remain byte-exact the separately reviewed components.
for p in ['src/project/cxx_owned_data.rs','tests/project/command_argument_borrow.rs','docs/PUBLIC-CXX-OWNED-DATA-PACKAGE-V1.md']:assert (R/p).read_bytes()==git('show',N+':'+p)
for p in ['src/cleanup_plan/replay.rs','src/cleanup_plan/replay/path_array.rs','src/cleanup_plan/replay/path_array/before.rs','src/cleanup_plan/replay/path_array/tests.rs','src/cleanup_plan/replay/path_array/main696.rs','src/cleanup_plan/replay/path_array/funded_tests.rs']:assert (R/p).read_bytes()==git('show',P+':'+p)
for p in ['src/cleanup_plan/replay/factored/canonical_shared.rs','src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs']:assert (R/p).read_bytes()==git('show',U+':'+p)
for p in ['src/cleanup_plan/replay/path_join.rs','src/cleanup_plan/replay/path_join/before_state_move.rs','src/cleanup_plan/replay/path_join/state_move_tests.rs']:assert (R/p).read_bytes()==git('show',S+':'+p)
state=json.loads((R/'docs/evidence/singleton-state-move-next-20261016/current-to-364d.json').read_text())[0];prior=restore(state,(R/state['source']).read_bytes());assert prior==git('show',C+':'+state['source'])==(R/state['original_archive']).read_bytes();prior=prior.decode()
for reg in json.loads((R/'docs/evidence/singleton-state-move-next-20261016/old-registration-adapters.json').read_text()):assert prior.count(reg)==1;prior=prior.replace(reg,'',1)
assert prior==(R/'src/cleanup_plan/replay/path_join/before_state_move.rs').read_text()
for script in ['docs/evidence/path-array-first-output-funding-20261016/verify-source.py','docs/evidence/alias-project-provider-fixture-847-20261016/verify-source.py']:subprocess.run(['python3',str(R/script)],cwd=R,check=True,stdout=subprocess.DEVNULL)
assert not git('diff',B,'--name-only','--diff-filter=M','--','docs/evidence/**/*.json').strip()
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/cleanup_plan/replay/branch_clone.rs','src/cleanup_plan/replay/branch_clone/inline_tests.rs']:assert (R/p).read_bytes()==git('show',B+':'+p)
helper=(R/'src/source_reflows_merge.rs').read_text();old=git('show',B+':src/source_reflows_merge.rs').decode();assert helper[helper.index('fn project('):helper.index('// Exact post-build')]==old[old.index('fn project('):old.index('// Exact post-build')]
assert 'let next = post_main0ab1_input(source, current);'in helper
new=['src/cleanup_plan/replay/path_join/before_state_move.rs','src/cleanup_plan/replay/path_join/state_move_tests.rs']
for owner in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 for p in new:assert (R/owner).read_text().count('/'+p+'"')==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
caps={}
for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line and not line.startswith('#'):
  p,n=line.split();caps[p]=int(n)
for p in git('diff',B,'--name-only','--','src','tests','crates').decode().splitlines():
 if p.endswith('.rs') and (R/p).is_file():assert len((R/p).read_text().splitlines())<=caps.get(p,1500),p
parser=R/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';ns={};exec(compile(parser.read_text(),str(parser),'exec'),ns);ns['R']=R;original=ns['parse'];loaded=[]
def capture(path,*a,**kw):
 path=Path(path).resolve();assert path.is_file(),str(path);loaded.append(str(path.relative_to(R)));return original(path,*a,**kw)
ns['parse']=capture
capture(R/'src/cleanup_plan/replay/path_join.rs',['cleanup_plan','replay','path_join'],[],'lib','semaprax')
capture(R/'src/project/cxx_owned_data.rs',['project','cxx_owned_data'],[],'lib','semaprax')
capture(R/'tests/project/command_argument_borrow.rs',['command_argument_borrow'],[],'test:project','semaprax',custom_path=True)
assert set(new)<=set(loaded)
checks=[{'package':k[0],'kind':k[1],'selector':k[2],**value}for k,value in ns['IDX'].items()];assert len([x for x in checks if '::state_move_tests::'in x['selector']])==2;assert len([x for x in checks if '::provider_refusal_tests::'in x['selector']])==2
(D/'recursive-module-binding.json').write_text(json.dumps({'mode':'SOURCE_ONLY_NOT_RUST_DISCOVERY','parser_sha256':sha(parser.read_bytes()),'loaded_sources':sorted(set(loaded)),'checks':checks},indent=2)+'\n')
proof={'status':'SOURCE_ONLY_PASS','whole_current_to3177_rows':count,'compact_bytes':(D/'current-to-3177.json').stat().st_size,'prior14_27_3_rows_replayed':[nff,ncut,nmain],'four_immutable_Main_semantic_and_formatting_inverses':True,'Main0ab1_three_changed_sources_exact':True,'Main_funded_files_exact_d201':True,'831_exact_3ae':True,'singleton_exact_edac':True,'complete_state_receiver_registration_only':True,'original90_runtime_fixtures_and_old_JSON_exact':True,'old_project_function_exact':True,'two_new_closed_state_children_all3_raw_joins':True,'recursive_loaded_files':len(set(loaded)),'module_caps_pass':True,'Rust_executed':False,'prior_proof_reuse':'Whole restored namespace equality; strict predecessor scripts remain historical, never weakened.'};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
