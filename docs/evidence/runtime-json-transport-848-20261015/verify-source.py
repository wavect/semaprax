"""Bounded current-source/whole9ccc namespace proof; no Rust/IO-test execution."""
from pathlib import Path
import argparse,hashlib,json,re,subprocess
P=argparse.ArgumentParser();P.add_argument('--repo',required=True,type=Path);R=P.parse_args().repo.resolve();B=Path(__file__).resolve().parent;baseline=json.loads((B/'baseline.json').read_text());H=baseline['head'];D=R/'docs/evidence/runtime-json-transport-848-20261015';g=lambda *a:subprocess.check_output(['git',*a],cwd=R);h=lambda b:hashlib.sha256(b).hexdigest();review=json.loads((B/'reviewed-transport-binding.json').read_text());root=(R/'src/source_reflows_merge.rs').read_text()
# Original89 literals become fixed enum references; every physical byte stays exact.
assert 'include_str!' not in root
inventory=(R/'src/source_reflows_merge/runtime_json/inventory.rs').read_text();pat=r'Self::(F\d\d|Custody) => Spec \{\s*path: concat!\(env!\("CARGO_MANIFEST_DIR"\), "([^"\n]+)"\),\s*length: (\d+),\s*sha256: \[([^\]]*)\],\s*\}'
specs={name:(path,int(length),bytes(int(v.strip()) for v in digest.split(',') if v.strip()).hex()) for name,path,length,digest in re.findall(pat,inventory,re.S)};assert len(specs)==90
for row in baseline['fixtures']:
 assert root.count('Fixture::'+row['fixture'])==1;assert specs[row['fixture']]==(row['literal_path'],row['length'],row['sha256'])
 with (R/row['source']).open('rb') as file:assert hashlib.file_digest(file,'sha256').hexdigest()==row['sha256']
assert sum(specs[x['fixture']][1] for x in baseline['fixtures'])==93590460
originals=inventory.split('const ORIGINAL_FIXTURES: [Fixture; 89] = [',1)[1].split('];',1)[0];assert re.findall(r'Fixture::(F\d\d)',originals)==[r['fixture'] for r in baseline['fixtures']]
# The old project body remains byte-exact. Only reviewed transport and explicit
# first frontiers differ; strip our additions and bind the independently reviewed root.
assert root[root.index('fn project('):root.index('// Exact post-build')]==(B/'original-project.rs.txt').read_text()
prefix='    let before_transport = before_runtime_json_transport(source, current);\n    let current = before_transport.as_str();\n';suffix='\n// Complete pre-transport physical inputs precede every older named view.\nfn before_runtime_json_transport(source: &str, current: &str) -> String {\n    project(\n        &runtime_json::load(runtime_json::Fixture::Custody),\n        source,\n        current,\n    )\n}\n';assert root.count(prefix)==2 and root.endswith(suffix);assert h(root[:-len(suffix)].replace(prefix,'').encode())==review['files']['src/source_reflows_merge.rs']
for name in ['final_batch_input','runtime848_input']:assert re.search(r'fn '+name+r'\([^\n]+\) -> String \{\n'+re.escape(prefix),root)
assert h((R/'src/source_reflows_merge/runtime_json.rs').read_bytes())==review['files']['src/source_reflows_merge/runtime_json.rs']
rows=json.loads((D/'current-to-9ccc.json').read_text());expected=set(baseline['whole_original_sources'])|{'docs/ARCHITECTURE.md','CHANGELOG.md'};assert len(rows)==6 and {r['source'] for r in rows}==expected;named={}
for row in rows:
 path=R/row['source'];current=path.read_bytes();assert h(current)==row['current_sha256'];old=current
 for edit in row['edits']:assert old.count(edit['new'].encode())==1;old=old.replace(edit['new'].encode(),edit['old'].encode(),1)
 assert h(old)==row['original_sha256'] and old==(R/row['archive']).read_bytes()==g('show',H+':'+row['source']);named[row['source']]=old
assert (D/'source-inverses.json').read_bytes()==(D/'current-to-9ccc.json').read_bytes();data=(D/'current-to-9ccc.json').read_bytes();assert specs['Custody']==('/docs/evidence/runtime-json-transport-848-20261015/current-to-9ccc.json',len(data),h(data));assert len(data)<=5081115
# Every changed pre-existing Git input is restored. New children have explicit
# new-source custody in all three audits, not a false claim of prior existence.
tracked=set(g('ls-tree','-r','--name-only',H).decode().splitlines())
for p in g('diff','--name-only',H).decode().splitlines():
 if p in tracked:assert p in expected and named[p]==g('show',H+':'+p),p
owners=['src/source_reflows_merge/runtime_json.rs','src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/tests.rs']
for source in list(baseline['whole_original_sources'])[1:]:
 text=(R/source).read_text()
 for owner in owners:assert text.count(owner[4:]+'"')==1,(source,owner)
assert len((R/list(baseline['whole_original_sources'])[-1]).read_text().splitlines())==1500
for owner in owners:assert len((R/owner).read_text().splitlines())<=1500
assert len(root.splitlines())<=1500
# All old existing inputs including docs restore exactly before frozen848/MAIN/
# doc/branch joins: prior authenticated namespace proofs can be reused unchanged.
proof={'status':'SOURCE_ONLY_TRANSPORT_CUSTODY_NOT_EXECUTED','source_basis':H,'old_complete_project_body_byteexact':True,'reviewed_transport_root_restored_byteexact':True,'original_fixture_count':89,'original_fixture_bytes':93590460,'all_original_file_SHA256_exact':True,'extra_custody_fixture_count':1,'extra_custody_fixture_bytes':len(data),'runtime_fixture_count':90,'whole9ccc_preexisting_input_inverses':6,'new_child_sources_all_three_audits':owners,'new_child_hashes_metadata_only':{p:h((R/p).read_bytes()) for p in owners},'inherited_original_namespace_exact_for_proof_reuse':True,'old_JSON_scripts_archives_unchanged':True,'repository_runtime_fixture_required':True,'RAM_or_runtime_gain_claim':False,'Rust_Cargo_IO_test_execution':False}
(D/'custody-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
