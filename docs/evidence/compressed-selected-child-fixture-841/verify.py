#!/usr/bin/env python3
"""Bounded source/fixture checks only; not Rust execution or RSS measurement."""
from pathlib import Path
import gzip,io,json,hashlib,re,subprocess,tomllib
D=Path(__file__).resolve().parent;R=D.parents[2];P=json.loads((D/'source-inverses.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
for row in P['sources']:
 old=subprocess.check_output(['git','show',P['base']+':'+row['path']]);current=(R/row['path']).read_bytes()
 assert old==row['before'].encode() and sha(old)==row['before_sha256']
 assert current==row['current'].encode() and sha(current)==row['current_sha256']
for row in P['additions']:
 b=(R/row['path']).read_bytes();assert b==row['source'].encode() and sha(b)==row['sha256']
M=json.loads((D/'fixture-metadata.json').read_text());raw=(R/M['raw_path']).read_bytes();assert raw==subprocess.check_output(['git','show',P['base']+':'+M['raw_path']]);assert len(raw)==M['raw_bytes']==5196870 and sha(raw)==M['raw_sha256']
z=(D/'exact-source-inverses.json.gz').read_bytes();assert len(z)==M['gzip_bytes']==483941 and sha(z)==M['gzip_sha256'];assert gzip.decompress(z)==raw
out=io.BytesIO()
with gzip.GzipFile(filename='',mode='wb',fileobj=out,compresslevel=6,mtime=0) as g:g.write(raw)
assert out.getvalue()==z and z[4:8]==b'\0'*4 and z[9]==255
helperpath=P['additions'][0]['path'];helper=(R/helperpath).read_text()
constants={int(n):bytes(map(int,re.findall(r'\d+',v))) for n,v in re.findall(r'const CURRENT_SHA_(\d+): \[u8; 32\] = \[(.*?)\];',helper,re.S)}
views=json.loads((D/'complete-source-views.json').read_text())['views'];assert len(views)==5
for n,v in enumerate(views):
 current=(R/v['path']).read_bytes();checkpoint=(D/v['checkpoint_archive']).read_bytes()
 assert current==v['current_bytes'].encode() and sha(current)==v['current_sha256'] and hashlib.sha256(current).digest()==constants[n]
 assert checkpoint==v['checkpoint_bytes'].encode() and sha(checkpoint)==v['checkpoint_sha256']
 assert checkpoint==subprocess.check_output(['git','show',v['checkpoint_head']+':'+v['path']])
rows=json.loads(raw);mapping={v['path']:v for v in views};base_failures=[]
for row in rows:
 current=(R/row['source']).read_text()
 if row['source'] in mapping:
  v=mapping[row['source']];assert sha(current.encode())==v['current_sha256'];current=v['checkpoint_bytes']
 for e in row['edits']:
  assert current.count(e['new'])==1,row['source'];current=current.replace(e['new'],e['before'],1)
 assert current==row['original_bytes'],row['source']
 # Record original851 drift; never relabel historical baseline as passing.
 b=subprocess.check_output(['git','show',P['base']+':'+row['source']],text=True)
 for e in row['edits']:
  if b.count(e['new'])!=1:base_failures.append(row['source']);break
  b=b.replace(e['new'],e['before'],1)
assert len(base_failures)==4
source=R/'src/cleanup_plan/replay/selected_context/child/tests/source.rs';s=source.read_text();original=(D/'source.rs.before.txt').read_text()
s=s.replace('#[path = "source/compressed_fixture.rs"]\nmod compressed_fixture;\n','')
a='''    // Decode before the original JSON/source assertions, outside every measured route.
    let decoded = compressed_fixture::load();
    let rows: serde_json::Value = serde_json::from_str(&decoded).unwrap();'''
b='''    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../docs/evidence/selected-child-refusal-781/exact-source-inverses.json"
    ))
    .unwrap();'''
assert s.count(a)==1;s=s.replace(a,b).replace('        let current = compressed_fixture::prior_source(path, current);\n','');assert s==original,'whole original assertion owner restored'
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 assert (R/p).read_text().count(helperpath+'"')==1
assert helper.index('if expected_len > OUTPUT_CAP')<helper.index('let mut output = Vec::new()')<helper.index('GzDecoder::new(bytes)')
assert '.read_to_end(' not in helper and 'OnceLock' not in helper and 'LazyLock' not in helper
old=tomllib.loads(subprocess.check_output(['git','show',P['base']+':Cargo.lock'],text=True));new=tomllib.loads((R/'Cargo.lock').read_text());key=lambda p:(p['name'],p['version'],p.get('source'));a={key(p):p for p in old['package']};b={key(p):p for p in new['package']};assert not(a.keys()-b.keys())
for k,p in a.items():
 if p!=b[k]:
  assert p['name']=='semaprax';expected=dict(p);expected['dependencies']=sorted(p['dependencies']+['flate2']);assert expected==b[k]
assert {p['name'] for k,p in b.items() if k not in a}=={'adler2','crc32fast','flate2','miniz_oxide','simd-adler32'}
deps=tomllib.loads((R/'Cargo.toml').read_text());assert 'flate2' not in deps['dependencies'];assert deps['dev-dependencies']['flate2']=={'version':'=1.1.10','default-features':False,'features':['rust_backend']}
for row in P['sources']+P['additions']:
 if row['path'].endswith('.rs'):assert len((R/row['path']).read_text().splitlines())<=1500,row['path']
result=dict(status='SOURCE_ONLY_PASS',raw_fixture_unchanged=True,raw_bytes=len(raw),compressed_bytes=len(z),all_original_assertion_owner_bytes_restored=True,full_original_inverse_rows_replayed=len(rows),baseline851_preexisting_failed_inputs=base_failures,new_full_authenticated_views=5,source_audit_joins=3,lock_existing_versions_unchanged=True,Rust_execution=False,paired_compile_or_RSS_measurement=False,issue841='OPEN_PENDING_RUNTIME_AND_MEANINGFUL_PAIRED_MEASUREMENT')
print(json.dumps(result,indent=2))
