from pathlib import Path
import hashlib,json,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='5aacb8e5e652ce4d05bc8717d7277145695167a7'
sha=lambda x:hashlib.sha256(x).hexdigest()
def git(*x):return subprocess.check_output(['git',*x],cwd=R)
rows=json.loads((D/'current-to-5aac.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text())
view={}
for row in rows:
 p=row['source'];old=git('show',B+':'+p);raw=(R/p).read_bytes();assert sha(raw)==row['current_sha256'];text=raw.decode()
 for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert text.encode()==old==(R/row['archive']).read_bytes();assert sha(old)==row['original_sha256'];view[p]=old
modified=git('diff',B,'--name-only','--diff-filter=M').decode().splitlines();assert {r['source']for r in rows}==set(modified);assert not [p for p in modified if p.startswith('docs/evidence/')]
# Every protected predecessor JSON/archive/script is byte-exact by full Git diff.
for p in ['src/source_reflows_merge/runtime_json.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/source_reflows_merge/runtime_json/tests.rs']:assert (R/p).read_bytes()==git('show',B+':'+p)
helper=(R/'src/source_reflows_merge.rs').read_text();oldhelper=git('show',B+':src/source_reflows_merge.rs').decode()
a=helper.index('fn project(');b=helper.index('// Exact post-build',a);c=oldhelper.index('fn project(');d=oldhelper.index('// Exact post-build',c);assert helper[a:b]==oldhelper[c:d]
assert helper.count('    let compile_fixed = compile_fixture_input(source, current);')==2
root='src/cleanup_plan/replay.rs';assert sha(view[root])=='5996c732aceb53c6f4ae7a8924ea020584328b2d86638446bbd3f365e925d503'
before=(R/'src/cleanup_plan/replay/branch_clone/before.rs').read_text();assert 'let compile_fixed = crate::source_reflows_merge::compile_fixture_input("src/cleanup_plan/replay.rs", current);' in before
# Prefix restoration proves the entire frozen5aac prior input namespace, not a new runtime result.
added=[p for p in git('diff',B,'--name-only','--diff-filter=A','--','src','tests').decode().splitlines()if p.endswith('.rs')]
for n in added:
 for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert (R/p).read_text().count('/'+n+'")')==1,(p,n)
 assert len((R/n).read_text().splitlines())<=1500
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
proof={'status':'SOURCE_ONLY_PASS','before':B,'whole_old_inputs':len(rows),'compact_input_bytes':(D/'current-to-5aac.json').stat().st_size,'new_live_Rust_owners':len(added),'old90_runtime_transport_exact':True,'original_project_body_exact':True,'full_prior_namespace_restored':True,'Rust_executed':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
