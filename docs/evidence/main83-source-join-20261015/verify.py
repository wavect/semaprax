from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;H='4f5b5ccfad20ce100205d6cf666c1b6c27330818';M='83f069e4c9500bc9bac7e0f3a1ba86f7dbef9c8d';g=lambda *a:subprocess.check_output(['git',*a],cwd=R);h=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'current-to-4f5b.json').read_text());assert len(rows)==7;views={}
for row in rows:
 s=row['source'];current=(R/s).read_bytes();assert h(current)==row['current_sha256'];old=current
 for edit in row['edits']:assert old.count(edit['new'].encode())==1;old=old.replace(edit['new'].encode(),edit['old'].encode(),1)
 assert h(old)==row['original_sha256'] and old==(R/row['archive']).read_bytes()==g('show',H+':'+s);views[s]=old
tracked=set(g('ls-tree','-r','--name-only',H).decode().splitlines())
for s in g('diff','--name-only',H).decode().splitlines():
 if s in tracked:assert s in views and views[s]==g('show',H+':'+s),s
assert (D/'source-inverses.json').read_bytes()==(D/'current-to-4f5b.json').read_bytes()
for s in ['src/workspace_graph/tests/record_traversal.rs','tests/project_candidate/string_builtin_calls.rs']:assert (R/s).read_bytes()==g('show',M+':'+s)
folder='docs/evidence/canonical-preparation-stage-829/retry-precharge-fixture/'
for s in g('diff','--name-only','f4a448da6ed71da57719ea7bc8062b134484e8a7',M).decode().splitlines():
 if s.startswith(folder):assert (R/s).read_bytes()==g('show',M+':'+s)
inv=json.loads((R/(folder+'exact-inverse.json')).read_text());before=(R/(folder+'exact-inverse.before-rustfmt.json')).read_bytes();assert before==g('show',H+':'+folder+'exact-inverse.json');prior=json.loads(before);assert len(inv['splices'])==38 and inv['splices'][:3]==prior['splices'] and len(prior['splices'])==3
builder=views['src/workspace_graph/tests/builder_limits.rs'];start=builder.index(b'/// Tune only the fixture size, never the production bound.');end=builder.index(b'\n#[test]\nfn identity_prebound_production_core_retry',start);fragment=builder[start:end];assert h(fragment)=='a1d1805d9ac00293249b4323ea54127c2961a37d4c92da51bba1a4915cfa6c54'
for splice in reversed(inv['splices']):
 at=splice['start'];after=splice['after'].encode();assert fragment[at:at+len(after)]==after;fragment=fragment[:at]+splice['before'].encode()+fragment[at+len(after):]
assert fragment==(R/(folder+'complete-fixture.before.txt')).read_bytes() and h(fragment)=='4c8f8a2cac574013d6acb485d7af2c86699fb3b5b09be532c90786c6ec7add24'
# All90 authenticated runtime fixtures, loader and child owners remain untouched.
source=g('show',H+':src/source_reflows_merge/runtime_json/inventory.rs').decode();import re
paths=re.findall(r'"(/docs/evidence/[^"\n]+\.json)"',source);assert len(paths)==90
for s in paths:assert (R/s.lstrip('/')).read_bytes()==g('show',H+':'+s.lstrip('/'))
for s in ['src/source_reflows_merge/runtime_json.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/tests.rs']:assert (R/s).read_bytes()==g('show',H+':'+s)
helper=(R/'src/source_reflows_merge.rs').read_text();old=g('show',H+':src/source_reflows_merge.rs').decode();a=helper.index('pub(crate) fn retry_fixture_input(');b=helper.index('// Every older runtime source guard',a);assert helper[a:b]==old[old.index('pub(crate) fn retry_fixture_input('):old.index('// Every older runtime source guard')]
assert helper[helper.index('fn project('):helper.index('// Exact post-build')]==old[old.index('fn project('):old.index('// Exact post-build')]
caller='crate::source_reflows_merge::main83_retry_fixture_input(';assert (R/'src/workspace_graph/tests/builder_limits.rs').read_text().replace(caller,'crate::source_reflows_merge::retry_fixture_input(')==views['src/workspace_graph/tests/builder_limits.rs'].decode();assert len(helper.splitlines())<1500
proof={'status':'SOURCE_ONLY_MAIN83_JOIN_NOT_EXECUTED','parents':[H,M],'complete_old4f_namespace_rows':7,'Main_changed_Rust_exact':2,'original89_runtime_files_bytes':93590460,'extra_runtime_Custody_files':1,'extra_runtime_Custody_bytes':51472,'all90_runtime_files_unchanged':True,'new_compiled_projection_count':1,'new_compiled_projection_bytes':(D/'current-to-4f5b.json').stat().st_size,'Main38_splices_preserve_old3':True,'actual_formatted_fragment_restores_full_frozen_original':True,'old_F56_helper_and_table_preserved':True,'new_reader_independent_formatted_domain':True,'all_old_runtime_assertions_unchanged':True,'production_unchanged':True,'Cargo_or_Rust_or_IOtest_execution':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
