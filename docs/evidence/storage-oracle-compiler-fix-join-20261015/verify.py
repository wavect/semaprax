from pathlib import Path
import hashlib,json,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
H='d719ea241120655b618ca85a185d5fe5c575a2bf';M='d6caf6979364b91c51f481fd02fb1e5417bc3ca5'
g=lambda *a:subprocess.check_output(['git',*a],cwd=R);sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-d719.json').read_text());assert len(rows)==11;views={}
for row in rows:
 s=row['source'];b=(R/s).read_bytes();assert sha(b)==row['current_sha256']
 for e in row['edits']:
  new=e['new'].encode();assert b.count(new)==1;b=b.replace(new,e['old'].encode(),1)
 assert b==g('show',H+':'+s)==(R/row['archive']).read_bytes() and sha(b)==row['original_sha256'];views[s]=b
tracked=set(g('ls-tree','-r','--name-only',H).decode().splitlines())
for s in g('diff','--name-only',H).decode().splitlines():
 if s in tracked:assert s in views,s
assert (D/'source-inverses.json').read_bytes()==(D/'current-to-d719.json').read_bytes()
main_sources=['src/cleanup_plan/replay/factored/continuation_receiver.rs','src/cleanup_plan/replay/factored/continuation_receiver/tests.rs','src/cleanup_plan/replay/factored/continuation_receiver/before.rs','src/cleanup_plan/replay/factored/oracle_hook.rs']
for s in main_sources:
 b=(R/s).read_bytes()
 if s.endswith('continuation_receiver/tests.rs'):
  for new,old in [(b'assert_eq!(a.moves(), b.moves());',b'assert_eq!(a.moves, b.moves);'),(b'assert_eq!(a.epsilon(), b.epsilon());',b'assert_eq!(a.epsilon, b.epsilon);')]:
   assert b.count(new)==1;b=b.replace(new,old,1)
 assert b==g('show',M+':'+s),s
base='docs/evidence/storage-oracle-refusal-v1/'
for s in g('ls-tree','-r','--name-only',M,base).decode().splitlines():assert (R/s).read_bytes()==g('show',M+':'+s)
archive=(R/(base+'continuation_receiver.before.rs.txt')).read_bytes();assert archive==views[main_sources[0]]
assert (R/main_sources[2]).read_bytes()==archive.removesuffix(b'#[cfg(test)]\nmod tests;\n')
# Exactly one old-only-helper spelling and one ordered Drop compiler correction.
p='src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs';s=(R/p).read_bytes();assert s.replace(b'drop((actual, expected, layout, view, owner));\n        drop(registry);',b'drop((actual, expected, layout, view, owner, registry));',1)==views[p]
p='src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs';s=(R/p).read_bytes();assert s.replace(b'crate::workspace_graph::expected_projection::borrowed_imports::pay_default_teardown(&ty, &index)',b'super::super::super::pay_default_teardown(&ty, &index)',1)==views[p]
oldhelper=views['src/source_reflows_merge.rs'].decode();helper=(R/'src/source_reflows_merge.rs').read_text();assert helper[helper.index('fn project('):helper.index('// Exact post-build')]==oldhelper[oldhelper.index('fn project('):oldhelper.index('// Exact post-build')]
paths=re.findall(r'"(/docs/evidence/[^"\n]+\.json)"',(R/'src/source_reflows_merge/runtime_json/inventory.rs').read_text());assert len(paths)==90
for p in paths:assert (R/p.lstrip('/')).read_bytes()==g('show',H+':'+p.lstrip('/'))
for p in ['src/source_reflows_merge/runtime_json.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/tests.rs']:assert (R/p).read_bytes()==g('show',H+':'+p)
child='src/cleanup_plan/replay/factored/continuation_receiver/before.rs'
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 assert (R/p).read_text().count(child)==1 and len((R/p).read_text().splitlines())<=1500
proof={'mode':'SOURCE_ONLY_NO_CARGO_OR_RUST_TESTS','basis':H,'MAIN':M,'complete_current_to_d719_rows':11,'MAIN_receiver_files_exact':3,'MAIN_test_file_exact_except_two_existing_compact_node_accessors':True,'original90_runtime_files_unchanged':True,'compiler_fixes_exactly_two_sites':True,'audit_includes':3,'interop_lines':len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines()),'compiled_outer_table_bytes':(D/'current-to-d719.json').stat().st_size}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
