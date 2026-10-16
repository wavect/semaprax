"""Finite runtime transport/custody proof; no Rust or measurement."""
from pathlib import Path
import subprocess,json,hashlib,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='e204ec37dd0102c77da46139d106086e279e6c3e'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
rows=json.loads((D/'current-to-e204.json').read_text());assert len(rows)==9
restored={}
for row in rows:
 p=row['source'];b=(R/p).read_bytes();assert sha(b)==row['current_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1,p;s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'];assert s.encode()==git('show',B+':'+p)==(R/row['archive']).read_bytes();restored[p]=s.encode()
# Entire old component packet and every old90 inventory/transport source stay exact.
assert not git('diff',B,'--name-only','--','docs/evidence/bulk-transport-pair-848-next-20261016').strip()
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','src/source_reflows_merge/runtime_json/pair_custody.rs','src/source_reflows_merge/runtime_json/transport_runtime.rs','src/source_reflows_merge/runtime_json/transport_embedded.rs','tests/support/production_source_inventory.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p)
fixtures=json.loads((D/'fixture-inventory.json').read_text());mapping=json.loads((D/'replacement-map.json').read_text());assert len(fixtures)==36 and len(mapping)==38
assert sum(x['length']for x in fixtures)==2612409
new=(R/'src/source_reflows_merge/metadata_json.rs').read_text();root=(R/'src/source_reflows_merge.rs').read_text();old=git('show',B+':src/source_reflows_merge.rs').decode()
for f in fixtures:
 b=(R/f['path'].lstrip('/')).read_bytes();assert len(b)==f['length']and sha(b)==f['sha256']
 data=json.loads(b);keys=sorted(x['source']for x in data if isinstance(x,dict)and isinstance(x.get('source'),str));assert keys==f['keys']
 assert '\0'not in keys
 # Only path/length/hash/key metadata is compiled, never any of these payloads.
 assert new.count('"'+f['path']+'"')==1
 assert 'include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "'+f['path']+'"))'not in new
 assert root.count('metadata_json::project(metadata_json::Fixture::'+f['variant']+',')==sum(x['variant']==f['variant']for x in mapping)
for m in mapping:assert old.count(m['literal'])>=1 and m['literal']not in root
assert 'include_bytes!'not in new
c=json.loads((D/'arm-contract.json').read_text());common=(R/c['selector_source']).read_bytes();embedded=common.replace(c['runtime_literal'].encode(),c['embedded_literal'].encode(),1);assert common.count(c['runtime_literal'].encode())==1
for n,b,key in [('COMMON_ARM',common,'common_source_sha256'),('EMBEDDED_ARM',embedded,'embedded_source_sha256')]:
 assert sha(b)==c[key];v=re.search(r'const '+n+r': \[u8; 32\] = \[(.*?)\];',new,re.S)[1];assert bytes(int(x.strip())for x in v.split(',')if x.strip())==hashlib.sha256(b).digest()
# Both selector arms normalize before this predecessor, then use the original
# e204 arm normalization/capsule unchanged. Never serially cross other domains.
p=c['selector_source'];assert common.replace(c['runtime_literal'].encode(),c['embedded_literal'].encode(),1).replace(c['embedded_literal'].encode(),c['runtime_literal'].encode(),1)==common
oldrows=json.loads((R/'docs/evidence/bulk-transport-pair-848-next-20261016/current-to-0c.json').read_text())
for row in oldrows:
 p=row['source'];b=restored.get(p,(R/p).read_bytes());assert sha(b)==row['current_sha256']
loader=(R/'src/source_reflows_merge/runtime_json.rs').read_text();prior=git('show',B+':src/source_reflows_merge/runtime_json.rs').decode();assert loader[loader.index('fn read_fixture('):]==prior[prior.index('fn read_fixture('):]
test=(R/'src/source_reflows_merge/runtime_json/tests.rs').read_text();oldtest=git('show',B+':src/source_reflows_merge/runtime_json/tests.rs').decode();a=test.index('    let path = std::env::temp_dir()');assert test[a:]==oldtest[oldtest.index('    let path = std::env::temp_dir()'):]
def project(s):a=s.index('fn project(');return s[a:s.index('fn before_build_fix',a)]
assert project(root)==project(old)
assert 'let metadata = metadata_json::previous(source, current);'in root
assert 'metadata_json::normalize(source, current)'in root
owners=json.loads((D/'new-owners.json').read_text());assert len(owners)==1
for row in owners:
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256']and len(b.splitlines())==row['lines']<=1500
 for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert row['source']in(R/p).read_text()
assert not [p for p in git('diff',B,'--name-only','--diff-filter=M').decode().splitlines()if p.endswith('.json')]
proof={'mode':'SOURCE_ONLY_NO_RUST_NO_RAM_CLAIM','full_e204_inverse_rows':9,'fixed_metadata_files':36,'fixed_unique_bytes':2612409,'replaced_literal_occurrences':38,'old90_packet_and_inventory_exact':True,'old90_all_file_loop_and_hostile_tail_exact':True,'new_owners':1,'pure_project_and_full_reader_tail_exact':True,'both_virtual_arms_share_same36_context':True}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
