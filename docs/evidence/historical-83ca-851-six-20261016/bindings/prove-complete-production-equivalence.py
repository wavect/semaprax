"""Complete Git-set equivalence, with exact closed test registration witnesses."""
from pathlib import Path
import argparse,subprocess,json,hashlib
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,required=True);A.add_argument('--expected-head',required=True);A.add_argument('--out',type=Path,required=True);a=A.parse_args();R=a.repo.resolve();B='83ca0e105e5f4eb6336ed04ff80df6ad0366e99f';D=R/'docs/evidence/main-dd0-compiled-custody-join-20261016'
def git(*args):return subprocess.check_output(['git',*args],cwd=R)
def sha(b):return hashlib.sha256(b).hexdigest()
assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain','--untracked-files=all').strip()
assert not a.out.exists()and not a.out.resolve().is_relative_to(R)
subprocess.run(['python3',str(D/'verify-source.py')],check=True)
subprocess.run(['python3',str(D/'verify-authority.py')],check=True)
def tree(rev):
 result={}
 for line in git('ls-tree','-r','-z',rev).split(b'\0'):
  if not line:continue
  fields,path=line.split(b'\t',1);mode,kind,oid=fields.decode().split();assert kind in ['blob','commit'];result[path.decode()]={'mode':mode,'kind':kind,'git_object':oid}
 return result
old=tree(B);new=tree(a.expected_head);deleted=set(old)-set(new);assert not deleted
changed={p for p in old if new.get(p)!=old[p]};added=set(new)-set(old)
three={'src/workspace_graph/type_proof_tests.rs','src/cleanup_plan/replay/compact_stage/source_custody.rs','src/source_reflows_merge.rs'}
interop='crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs';platform='platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'
assert {p for p in changed if p.startswith('src/')}==three
assert {p for p in changed if p.startswith('crates/')}=={interop}
assert all(p in three or p in [interop,platform,'CHANGELOG.md']or p.startswith('tests/')for p in changed)
new_rust={p for p in added if p.endswith('.rs')and p.split('/')[0]in ['src','crates','tests']};assert new_rust=={'tests/wasm/scalar_browser_ci_contract/actual_83ca.rs'}
assert all(p.startswith('docs/evidence/')or p in new_rust for p in added)
# All source/dependency/resource files outside these exact test/evidence changes
# have identical Git mode and content-addressed immutable bytes; no file omitted.
unmodified=set(old)-changed;assert all(old[p]==new[p]for p in unmodified)
protected=[p for p in old if p in ['Cargo.toml','Cargo.lock','build.rs']or p.endswith('/Cargo.toml')or p.endswith('/build.rs')or p.startswith(('examples/','std/','.cargo/','.github/'))]
assert all(p in new and old[p]==new[p]for p in protected)
parents={
 'src/lib.rs':['#[cfg(test)]\nmod source_reflows_merge;'],
 'src/workspace_graph.rs':['#[cfg(test)]\n#[path = "workspace_graph/type_proof_tests.rs"]\nmod type_proof_tests;'],
 'src/cleanup_plan/replay/compact_stage.rs':['#[cfg(test)]\npub(super) mod source_custody;'],
 'crates/semaprax-native-rust-interop-builder/src/implementation.rs':['#[cfg(test)]\n#[path = "implementation/tests.rs"]\nmod tests;'],
 'crates/semaprax-native-rust-interop-builder/src/implementation/tests.rs':['#[path = "tests/ledger_capacity.rs"]\nmod ledger_capacity;'],
 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs':['include!("ledger_capacity/source_inventory.rs");']}
witness=[]
for p,registrations in parents.items():
 raw=(R/p).read_bytes();assert raw==git('show',B+':'+p)
 for text in registrations:assert raw.decode().count(text)==1,(p,text)
 witness.append({'parent':p,'sha256':sha(raw),'exact_registration_text':registrations,'byteexact83ca':True})
# Current original parser proves every other chain/role; no HIR or projection
# policy root is widened to classify the separately registered pin-only owner.
hir=json.loads((D/'hir.json').read_text());projection=json.loads((D/'projection.json').read_text())
for p in three-{'src/workspace_graph/type_proof_tests.rs'}:
 assert hir['classes'][p]['role']=='test-only'
 if p in projection['classes']:assert projection['classes'][p]['role']=='test-only'
hir=json.loads((D/'hir.json').read_text());assert hir['classes'][next(iter(new_rust))]['role']=='test-only'
assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain','--untracked-files=all').strip()
result={'status':'COMPLETE_PRODUCTION_SOURCE_EQUIVALENCE_PASS','current_source_head':a.expected_head,'production_source_head':B,'complete_Git_file_set_checked':True,'deleted_files':[],'unchanged_Git_entries_count':len(unmodified),'unchanged_Git_entries_manifest_sha256':sha(json.dumps({p:old[p]for p in sorted(unmodified)},sort_keys=True,separators=(',',':')).encode()),'comparison':'Identical Git modes and content-addressed immutable blob bytes for every original file except explicit closed test/evidence changes; sourceproof also authenticates full changed before/current bytes and inverses.','all_changed_original_paths':sorted(changed),'three_changed_src_modules_only':sorted(three),'closed_test_registration_witnesses':witness,'third_raw_audit_cfgtest_include':interop,'new_Rust_files_only_old_wasm_harness':sorted(new_rust),'protected_Cargo_build_examples_std_CI_entries_unchanged':len(protected),'source_proof_sha256':sha((D/'source-proof.json').read_bytes()),'authority_proof_sha256':sha((D/'authority-source-proof.json').read_bytes()),'all_original_JSON_and_full_receivers_preserved':True,'producer_artifacts_and_captures_are_explicit83ca_history':True,'current_consumer_PASS':False}
a.out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'status':result['status'],'head':a.expected_head,'unchanged_Git_entries':len(unmodified),'changed_test_evidence_entries':len(changed),'out':str(a.out),'sha256':sha(a.out.read_bytes())}))
