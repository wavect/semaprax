"""Source-only custody authoring; no compiler, runtime, review, or paid call."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='e3eb476838442b58c02ccc815aae4bbdfebfd50e';I=subprocess.check_output(['git','rev-parse','2677c34b3'],cwd=R,text=True).strip()
sha=lambda b:hashlib.sha256(b).hexdigest()
owners=subprocess.check_output(['git','diff','--name-only','a4c91d7aa'],cwd=R,text=True).splitlines();owners=[p for p in owners if p.startswith(('src/codegen/','src/wasm/','tests/'))]
assert len(owners)==13,owners
children=['src/codegen/native_emit/expression/vec_ops/field_update.rs','src/codegen/native_vec/field_update.c','src/wasm/aggregate/vec_owned_leaf/field_update.rs','tests/support/record_copy_field_update.rs','tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs','tests/owned_data/owned_leaf_vec/copy_field_update.rs']
rows=[]
for path in owners:
 old=subprocess.check_output(['git','show',B+':'+path],cwd=R);current=(R/path).read_bytes();archive=D/'original'/path.replace('/','__');archive.write_bytes(old)
 rows.append({'source':path,'basis':B,'archive':str(archive.relative_to(R)),'original_sha256':sha(old),'current_sha256':sha(current),'edits':[{'new':current.decode(),'before':old.decode()}]})
packet={'basis':B,'dependency_original_interface_commit':I,'private_cherry_pick':'a4c91d7aa','dependency_frontend_continuation':'4855f3363e9f6db449433167e789c840203503d1','status':'SOURCE_AUTHORED_UNEXECUTED_NO_RUST_OR_CLAUDE','owners':rows,'children':[{'source':p,'sha256':sha((R/p).read_bytes())}for p in children]}
(D/'source-inverses.json').write_text(json.dumps(packet,indent=2)+'\n')
unchanged=['src/codegen/native_vec/owned_leaf.c','src/codegen/native_vec/legacy_field.c','src/codegen/native_vec/owned_payload.rs','tests/cleanup_backends/owned_leaf_collections/native.rs','tests/cleanup_backends/owned_leaf_collections/allocations.c']
proof=[]
for path in unchanged:
 old=subprocess.check_output(['git','show',B+':'+path],cwd=R);assert(R/path).read_bytes()==old;archive=D/'original'/path.replace('/','__');archive.write_bytes(old);proof.append({'source':path,'archive':str(archive.relative_to(R)),'sha256':sha(old)})
(D/'unchanged-originals.json').write_text(json.dumps(proof,indent=2)+'\n')
P=Path('/Volumes/Semaprax/provenance/qualification-registration-module-path-20261014.py');prefix=P.read_text().split("parse(R/'src/lib.rs'")[0];n={};exec(prefix,n);n['R']=R
for harness in ['cleanup_backends','owned_data']:n['parse'](R/('tests/'+harness+'.rs'),[],[],'test:'+harness,'semaprax')
new=[k for k in n['IDX']if '::copy_field_update::'in k[2]];assert len(new)==5,new
required=[]
for harness,sel in [('cleanup_backends','owned_leaf_collections::shared_owned_leaf_fixture_settles_at_o0_and_o2'),('cleanup_backends','owned_leaf_collections::cloned_record_remains_independent_after_clear_and_replacement'),('owned_data','owned_leaf_vec::field_reads::every_copy_field_shape_retains_its_scalar_bits'),('owned_data','owned_leaf_vec::field_reads::legacy_tag10_permuted_fields_are_bound_at_construction'),('owned_data','owned_leaf_vec::field_reads::index_refusal_and_index_evaluation_failure_keep_source_cleanup')]:required.append(('semaprax','test:'+harness,sel))
checks=[]
for role,keys in [('new',new),('required_existing',required)]:
 for key in keys:
  v=n['IDX'][key];assert not v['ignored'];source=v['source_path'];checks.append({'package':key[0],'harness':key[1],'selector':key[2],'source':source,'source_sha256':sha((R/source).read_bytes()),'source_binding':v,'role':role,'status':'SOURCE_AUTHORED_UNEXECUTED'})
(D/'pending-selectors.json').write_text(json.dumps({'basis':B,'dependency_interface':I,'status':'SOURCE_AUTHORED_UNEXECUTED','new':5,'required_existing':5,'checks':checks,'qualification':'No Rust compilation, C/Node/Wasm execution, or tests; frontend continuation required before batch'},indent=2)+'\n')
print('SOURCE ONLY authored13fullsource inverses,6children,5unchangedfulloldoracles,5new+5existing exactsourcecontrols')
