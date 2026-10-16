"""Finite component/virtual selector proof only. No Rust or arm application."""
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='0c942afa4004da34ec84639b3f0a2f6e97b52bb8'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
contract=json.loads((D/'arm-contract.json').read_text())
source=contract['selector_source'];common=(R/source).read_bytes();assert sha(common)==contract['common_source_sha256']
assert common.count(contract['runtime_literal'].encode())==1
embedded=common.replace(contract['runtime_literal'].encode(),contract['embedded_literal'].encode(),1)
assert sha(embedded)==contract['embedded_source_sha256']
child=(R/'src/source_reflows_merge/runtime_json/pair_custody.rs').read_text()
for name,raw in [('COMMON',common),('EMBEDDED',embedded)]:
 value=re.search(r'const '+name+r': \[u8; 32\] = \[(.*?)\];',child,re.S)[1]
 assert bytes(int(x.strip())for x in value.split(',')if x.strip())==hashlib.sha256(raw).digest()
rows=json.loads((D/'current-to-0c.json').read_text());assert len(rows)==9
for row in rows:
 p=row['source'];raw=(R/p).read_bytes();assert sha(raw)==row['current_sha256'];text=raw.decode()
 for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
 assert sha(text.encode())==row['original_sha256']
 assert text.encode()==git('show',B+':'+p)==(R/row['archive']).read_bytes()
 # Virtual embedded input reaches the same exact complete predecessor.
 if p==source:
  virtual=embedded.replace(contract['embedded_literal'].encode(),contract['runtime_literal'].encode(),1)
  assert virtual==raw
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs','tests/support/production_source_inventory.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p)
fixtures=json.loads((D/'fixture-inventory.json').read_text());assert len(fixtures)==90
embed=(R/'src/source_reflows_merge/runtime_json/transport_embedded.rs').read_text()
for f in fixtures:
 with (R/f['source']).open('rb')as stream:actual=hashlib.file_digest(stream,'sha256').hexdigest()
 assert actual==f['sha256'] and (R/f['source']).stat().st_size==f['length']
 assert embed.count('"/'+f['source']+'"')==1
 assert ('Fixture::'+f['variant']+' =>')in embed
assert len(re.findall(r'include_bytes!',embed))==90
assert sum(x['length']for x in fixtures)==93641932
# Original pure project and every old loader/hostile assertion survive.
root=(R/'src/source_reflows_merge.rs').read_text();old=git('show',B+':src/source_reflows_merge.rs').decode()
def projector(s):a=s.index('fn project(');return s[a:s.index('fn before_build_fix',a)]
assert projector(root)==projector(old)
assert 'let pair = runtime_json::pair_previous(source, current);'in root
loader=common.decode();oldloader=git('show',B+':'+source).decode()
assert loader[loader.index('fn read_fixture('):]==oldloader[oldloader.index('fn read_fixture('):]
assert 'super::project(&selected_transport::load(fixture), source, current)'in loader
assert 'let spec = fixture.spec();\n    read_fixture('in loader
new=json.loads((D/'new-owners.json').read_text());assert len(new)==3
for row in new:
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256'];assert len(b.splitlines())==row['lines']<=1500
 for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
  assert row['source']in (R/path).read_text()
assert not [p for p in git('diff',B,'--name-only','--diff-filter=M').decode().splitlines()if p.endswith('.json')]
proof={'mode':'SOURCE_ONLY_NO_RUST_NO_PAIR_EXECUTION','full_predecessor_rows':9,'fixed_files':90,'fixed_bytes':93641932,'new_owners':3,'virtual_arm_hashes_exact':True,'project_body_and_full_loader_tail_exact':True,'original_authority_checker_unchanged':True}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
