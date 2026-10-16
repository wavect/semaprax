from pathlib import Path
import hashlib,json,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='2d4b68d3a416cae5fba6ac08498cd56721878ba1';M='531bde1ac6b835b3b4e44e0d0b15f3a9fc5e6d07'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def apply(row,raw):
 assert sha(raw)==row['current_sha256'];text=raw.decode()
 for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert sha(text.encode())==row['original_sha256'];return text.encode()
rows=json.loads((D/'current-to-2d4b.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text());assert len(rows)==13
for row in rows:
 old=git('show',B+':'+row['source']);assert apply(row,(R/row['source']).read_bytes())==old==(R/row['archive']).read_bytes()
modified=git('diff',B,'--name-only','--diff-filter=M').decode().splitlines();assert set(modified)=={r['source']for r in rows}
# Only this actualMain README is an intentional old evidence edit. AllJSON,
# histories, scripts, fixtures and the fixed90 transport remain exact.
assert [p for p in modified if p.startswith('docs/evidence/')]==['docs/evidence/darwin-native-link-reference-v1/README.md']
for p in ['src/source_reflows_merge/runtime_json.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/source_reflows_merge/runtime_json/tests.rs']:assert (R/p).read_bytes()==git('show',B+':'+p)
for p in ['.github/workflows/ci.yml','scripts/ci-darwin-native-reference.py','tests/project_product_acceptance_ci_contract.rs','docs/evidence/darwin-native-link-reference-v1/README.md']:assert (R/p).read_bytes()==git('show',M+':'+p)
base='src/cleanup_plan/replay/path_join/';root=R/(base+'main_single_groups')
files={'mod.rs':base[:-1]+'.rs','before.rs':base+'before.rs','boxed_states.rs':base+'boxed_states.rs','boxed_states/tests.rs':base+'boxed_states/tests.rs','receiver_tests.rs':base+'receiver_tests.rs','grouping.rs':base+'grouping.rs','grouping/tests.rs':base+'grouping/tests.rs','grouping/source_custody.rs':base+'grouping/source_custody.rs'}
for dest,src in files.items():
 now=(root/dest).read_bytes();old=git('show',M+':'+src)
 if dest=='mod.rs':
  row=json.loads((D/'runtime-main-root.json').read_text())[0];assert apply(row,now)==old==(R/row['archive']).read_bytes()
 elif dest=='grouping/source_custody.rs':assert now.replace(b'let mut restored = super::super::main_root_input();',b'let mut restored = include_str!("../../path_join.rs").to_owned();')==old
 else:assert now==old,(dest,src)
 assert len(now.splitlines())<=1500
# Every originalMain body/assertion remains; the root adapter executes before
# the complete unchanged3 positional inverse and original key-producer checks.
assert (root/'grouping/tests.rs').read_bytes().count(b'#[test]')==3
assert (root/'grouping/source_custody.rs').read_bytes().count(b'#[test]')==2
mainroot=json.loads((D/'runtime-main-root.json').read_text())[0];restored=apply(mainroot,(root/'mod.rs').read_bytes()).decode();inv=json.loads((R/'docs/evidence/single-state-groups-v1/inverse.json').read_text())
for at,new,old in reversed(inv):assert restored[at:at+len(new)]==new;restored=restored[:at]+old+restored[at+len(new):]
assert restored==(R/'docs/evidence/single-state-groups-v1/path_join.before.rs.txt').read_text()
for row in json.loads((D/'shared-main-inputs.json').read_text()):assert sha((R/row['source']).read_bytes())==row['sha256'];assert (R/row['source']).read_bytes()==git('show',M+':'+row['source'])
def item(t,s):
 a=t.index(s);i=t.index('{',a);depth=1;j=i+1
 while depth:depth+=(t[j]=='{')-(t[j]=='}');j+=1
 return t[a:j]
for token in ['struct PathState {','enum ReturnedSource {','struct ReplayConditionalVariant {']:assert item((R/'src/cleanup_plan/replay.rs').read_text(),token)==item(git('show',M+':src/cleanup_plan/replay.rs').decode(),token)
# Production route is byte-exact before its sole test-only registration suffix.
current=(R/(base[:-1]+'.rs')).read_bytes();suffix=b'\n#[cfg(test)]\n#[path = "path_join/main_single_groups/mod.rs"]\nmod main_single_groups;\n';assert current==git('show',B+':'+base[:-1]+'.rs')+suffix
helper=(R/'src/source_reflows_merge.rs').read_text();oldhelper=git('show',B+':src/source_reflows_merge.rs').decode();a=helper.index('fn project(');b=helper.index('// Exact post-build',a);c=oldhelper.index('fn project(');d=oldhelper.index('// Exact post-build',c);assert helper[a:b]==oldhelper[c:d]
assert 'let previous = main531_previous(source, current);' in helper
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
for n in files:
 path=base+'main_single_groups/'+n
 for owner in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert (R/owner).read_text().count('/'+path+'")')==1,(owner,path)
proof={'status':'SOURCE_ONLY_PASS','before':B,'main':M,'whole_prior_inputs':len(rows),'compact_input_bytes':(D/'current-to-2d4b.json').stat().st_size,'new_Main_root_input_bytes':(D/'runtime-main-root.json').stat().st_size,'new_Rust_owners':8,'Main_group_test_bodies':5,'Main_other_old_controls_retained':True,'modern_production_exact':True,'fixed90_exact':True,'Rust_executed':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
