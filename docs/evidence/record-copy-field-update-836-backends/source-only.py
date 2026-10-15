"""Exact byte/static custody only. Does not execute Rust, C, Node or Wasm."""
from pathlib import Path
import hashlib,json,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest();packet=json.loads((D/'source-inverses.json').read_text())
assert len(packet['owners'])==13 and len(packet['children'])==6
for row in packet['owners']:
 current=(R/row['source']).read_bytes();old=(R/row['archive']).read_bytes()
 assert sha(current)==row['current_sha256'] and sha(old)==row['original_sha256']
 text=current.decode()
 for edit in row['edits']:
  assert text.count(edit['new'])==1;text=text.replace(edit['new'],edit['before'],1)
 assert text.encode()==old
for row in packet['children']:assert sha((R/row['source']).read_bytes())==row['sha256']
for row in json.loads((D/'unchanged-originals.json').read_text()):assert(R/row['source']).read_bytes()==(R/row['archive']).read_bytes()and sha((R/row['source']).read_bytes())==row['sha256']
native=(R/'src/codegen/native_emit/expression/vec_ops/field_update.rs').read_text();wasm=(R/'src/wasm/aggregate/vec_owned_leaf/field_update.rs').read_text();runtime=(R/'src/codegen/native_vec/field_update.c').read_text()
assert '[call.source, call.index, call.value]'in native
for text in ['field_update::position(','stage_bytes_call_argument(','emit_operation_failure_check()?','plan.apply_at(&expression.id)?']:assert text in native
assert native.index('self.emit_operation_failure_check()?')<native.index('{flag} = false;')<native.index('plan.apply_at(&expression.id)?')
assert wasm.index('self.emit_expr(call.source)')<wasm.index('self.emit_expr(call.index)')<wasm.index('self.emit_expr(call.value)')<wasm.index('self.leaf_preflight_index(')<wasm.index('self.apply_call_commit(')<wasm.index('self.clear_scalar(&owner)')
assert 'call.source.clone()'not in wasm and '[I64; 6]'in(R/'src/wasm/aggregate/vec_owned_leaf.rs').read_text()
assert runtime.index('spx_leaf_check(c, s, d)')<runtime.index('if (index >= s->len)')<runtime.index('memcpy(target, &bits, sizeof(bits));')<runtime.index('spx_vec_transfer_result(c, s, e, r)')
assert runtime.count('memcpy(')==1
for banned in ['malloc(','calloc(','realloc(','free(','spx_leaf_drop_row(','spx_leaf_clone(']:assert banned not in runtime
assert 'd->kinds[field] != kind'in runtime and 'kind > 8'in runtime
host=(R/'tests/owned_data/owned_leaf_vec/host.js').read_text()
for text in ['fieldUpdating','spx_vec_update_field_v1','value.identity!==identity||value.shape!==shape','row[slot]=word;','value.generation++;','updateBefore[k]','vectorValue(handle)!==undefined','copies!==copied||drops!==dropped','scalarAt(owned)']:assert text in host
assert 'mod field_update;'in(R/'src/wasm/aggregate/vec_owned_leaf.rs').read_text()
assert 'mod field_update;'in(R/'src/codegen/native_emit/expression/vec_ops.rs').read_text()
support=(R/'tests/support/record_copy_field_update.rs').read_text()
for row in packet['owners']+packet['children']:assert row['source']in support,('missing complete backend audit join',row['source'])
assert 'OLD_ROW_ORACLE'in support and 'vec_clone_at<Row>'in support and 'vec_replace<Row>'in support
assert 'c,index,{{index=1usize;9}},"scalar"'in support
assert 'corpus::assert_sources();'in(R/'tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs').read_text()
assert 'corpus::assert_sources();'in(R/'tests/owned_data/owned_leaf_vec/copy_field_update.rs').read_text()
for child in packet['children']:
 p=R/child['source']
 for ref in re.findall(r'include_str!\(\s*"([^"]+)"',p.read_text()):assert(p.parent/ref).is_file(),(p,ref)
pending=json.loads((D/'pending-selectors.json').read_text());assert pending['new']==5 and pending['required_existing']==5
for row in pending['checks']:
 assert sha((R/row['source']).read_bytes())==row['source_sha256'] and not row['source_binding']['ignored']
 assert 'fn '+row['source_binding']['name']+'('in(R/row['source']).read_text()
limits={}
for line in(R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line.strip()and not line.startswith('#'):a=line.split('\t');limits[a[0]]=int(a[1])
roots=[R/'src',R/'tests']
for crate in(R/'crates').iterdir():roots.extend([crate/'src',crate/'tests'])
counts={str(p.relative_to(R)):len(p.read_text().splitlines())for root in roots if root.is_dir()for p in root.rglob('*.rs')}
violations=[(p,n,limits.get(p,1500))for p,n in counts.items()if n>limits.get(p,1500)];assert not violations,violations
proof={'status':'SOURCE_ONLY_UNEXECUTED_NO_RUST_C_NODE_WASM_OR_REVIEW','basis':packet['basis'],'dependency_original_interface':packet['dependency_original_interface_commit'],'dependency_frontend_continuation':packet['dependency_frontend_continuation'],'backend_owners':13,'current_children':packet['children'],'complete_old_worker_and_probe_sources_unchanged':5,'all_new_sources_joined_in_shared_backend_source_guard':True,'registered_exact_new_bodies':5,'required_exact_existing_controls':5,'module_budget_violations':violations,'module_budget_rust_files':len(counts),'wasm_root_lines':counts['src/wasm/aggregate.rs'],'wasm_root_budget':limits['src/wasm/aggregate.rs'],'actual_backend_results':None,'allocation_or_application_gain_claim':False,'global_Source36_and_67packets':'Unchanged prior-head meaning; integrator composes once after adoption','qualification_dependency':'Frontend continuation required beyond interface commit before compiler/runtime batch'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS13fullsource inverses+6children+5oldfullworker/probe archives; complete sourceguard joins;5new+5exactexistingcontrols;allRustbudgets. SOURCE ONLY.')
