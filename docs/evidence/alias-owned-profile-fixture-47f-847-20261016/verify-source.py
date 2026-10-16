from pathlib import Path
import hashlib,json,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='47f80b0f7a06fde9b2388439a9c0bfe998a05af1';p='src/workspace_graph/function_alias/before_core/tests.rs';row=json.loads((D/'current-to-47f.json').read_text())[0];sha=lambda b:hashlib.sha256(b).hexdigest();old=subprocess.check_output(['git','show',B+':'+p],cwd=R);new=(R/p).read_bytes();assert sha(new)==row['current_sha256']and sha(old)==row['original_sha256']and old==(R/row['archive']).read_bytes();restored=new.decode()
for e in row['edits']:assert restored.count(e['new'])==1;restored=restored.replace(e['new'],e['old'],1)
assert restored.encode()==old
# Full original strings and positive assertion/body tail are byte-exact.
for name in ['MANIFEST','APP','APP_PROVIDER','TESTS']:
 pattern=r'const '+name+r': &str = (r#"[\s\S]*?"#);';a=re.search(pattern,old.decode())[1];b=re.search(pattern,new.decode())[1];assert a==b,name
start='    // No preparatory build/cache warming:';end='    drop((old_image, current_image, old, current));';assert old.decode()[old.decode().index(start):old.decode().index(end)+len(end)]==new.decode()[new.decode().index(start):new.decode().index(end)+len(end)]
assert new.decode().index('let scalar_manifest')>new.decode().index(end)
literal=re.search(r'MANIFEST\.replacen\(\s*("(?:[^"\\]|\\.)*"),\s*("(?:[^"\\]|\\.)*"),',new.decode());assert literal
assert json.loads(literal[1])=='version = "1.0.0"\n'
assert json.loads(literal[2])=='version = "1.0.0"\nprofile = "owned-data-api.v1"\n' 
assert 'Public Scalar Export Profile v1 does not admit generic function templates or instances'in new.decode()
for field in ['code','severity','message','path','span','help']:assert f'assert_eq!(errors[0].{field}, expected.{field});'in new.decode()
assert len(new.decode().splitlines())<1500
changed=subprocess.check_output(['git','diff',B,'--name-only','--diff-filter=M'],cwd=R).decode().splitlines();assert changed==[p],changed
proof={'status':'SOURCE_ONLY_PASS','complete_original_manifest_and_Vec_payload_exact':True,'full_positive_borrowed_route_sources_Graph_Image_tail_exact':True,'owned_data_profile_only_fixture_selection':True,'complete_original_scalar_W115_all6fields_after_positive_builds':True,'whole_current_to47f_inverse':True,'production_changed':False,'new_test_bodies':0,'Rust_executed':False};(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
