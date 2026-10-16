from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='7e9fae0af7dff14fbdc31e48af056a380989b06c';M='531bde1ac6b835b3b4e44e0d0b15f3a9fc5e6d07'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,b):
 assert sha(b)==row['current_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'];return s.encode()
rows=json.loads((D/'current-to-7e9f.json').read_text());assert len(rows)==6 and rows==json.loads((D/'source-inverses.json').read_text());views={}
for row in rows:
 p=row['source'];old=git('show',B+':'+p);assert restore(row,(R/p).read_bytes())==old==(R/row['archive']).read_bytes();views[p]=old
changed=git('diff',B,'--name-only','--diff-filter=M').decode().splitlines();assert set(changed)==set(views);assert not [p for p in changed if p.startswith('docs/evidence/')]
p='src/cleanup_plan/replay/path_join/main_single_groups/mod.rs';assert (R/p).read_bytes().replace(b'#[path = "../tests.rs"]\n',b'')==views[p]
shared=json.loads((D/'shared-original-tests.json').read_text());assert sha((R/shared['source']).read_bytes())==shared['sha256'];assert (R/shared['source']).read_bytes()==git('show',M+':'+shared['source'])
# Complete adaptedMainroot→7e→originalMain. Old13-row/1-row runtimeJSONs stay exact.
oldD=R/'docs/evidence/main531-independent-groups-20261016';row=json.loads((oldD/'runtime-main-root.json').read_text())[0];assert restore(row,views[p])==git('show',M+':src/cleanup_plan/replay/path_join.rs')
# Scope the original static module parser to this one independent namespace.
parser=R/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';ns={};exec(compile(parser.read_text(),str(parser),'exec'),ns);ns['R']=R;original=ns['parse'];loaded=[]
def capture(path,*a,**kw):
 path=Path(path).resolve();assert path.is_file(),str(path);loaded.append(str(path.relative_to(R)));return original(path,*a,**kw)
ns['parse']=capture
capture(R/p,['cleanup_plan','replay','path_join','main_single_groups'],[],'lib','semaprax',custom_path=True)
checks=[{'package':k[0],'kind':k[1],'selector':k[2],**v}for k,v in ns['IDX'].items()];assert len(checks)==14
assert len(set(loaded))==9;assert shared['source']in loaded
for name in ['before.rs','boxed_states.rs','boxed_states/tests.rs','grouping.rs','grouping/tests.rs','grouping/source_custody.rs','receiver_tests.rs']:
 assert 'src/cleanup_plan/replay/path_join/main_single_groups/'+name in loaded
helper=(R/'src/source_reflows_merge.rs').read_text();assert 'let fixed = main531_child_input(source, current);'in helper and 'let fixed = main531_child_input("'+p+'", current);'in helper
for owner in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert (R/owner).read_text().count('/'+shared['source']+'")')==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
binding={'status':'SOURCE_ONLY_PASS','scoped_parser_sha256':sha(parser.read_bytes()),'loaded_sources':loaded,'checks':checks,'actual_Rust_discovery':False};(D/'recursive-module-binding.json').write_text(json.dumps(binding,indent=2)+'\n')
proof={'status':'SOURCE_ONLY_PASS','predecessor':B,'whole_prior_inputs':6,'compact_bytes':(D/'current-to-7e9f.json').stat().st_size,'full_shared_Main_tests':True,'recursive_Main_module_files':9,'literal_Main_test_bodies':14,'new_authored_test_bodies':0,'old_7e_JSON_and_evidence_exact':True,'new_namespace_to_Main_root_complete':True,'Rust_executed':False};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
