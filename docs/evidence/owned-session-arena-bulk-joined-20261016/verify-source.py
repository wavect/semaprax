#!/usr/bin/env python3
"""Finite source/fixture proof. No Rust execution, admission, or RAM claim."""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);a=p.parse_args();R=a.repo.resolve();D=R/'docs/evidence/owned-session-arena-bulk-joined-20261016';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*args):return subprocess.check_output(['git',*args],cwd=R)
B=json.loads((D/'source-bindings.json').read_text());C=json.loads((D/'arm-contract.json').read_text())
def normalize(path,b):
 if path!=C['selector_source']:return b
 if sha(b)==C['common_source_sha256']:return b
 assert sha(b)==C['embedded_source_sha256'],'complete final source-arm drift'
 assert b.count(C['embedded_literal'].encode())==1
 b=b.replace(C['embedded_literal'].encode(),C['runtime_literal'].encode(),1)
 assert sha(b)==C['common_source_sha256'];return b
assert sha((R/C['final_normalizer']).read_bytes())==C['final_normalizer_sha256']
runtime=(R/C['selector_source']).read_bytes();common=normalize(C['selector_source'],runtime);assert common.count(C['runtime_literal'].encode())==1
assert sha(common.replace(C['runtime_literal'].encode(),C['embedded_literal'].encode(),1))==C['embedded_source_sha256']
count=0
for label,desc in B['capsules'].items():
 data=(D/desc['path']).read_bytes();assert sha(data)==desc['sha256'];rows=json.loads(data);assert len(rows)==desc['rows'];assert len({r['source']for r in rows})==len(rows)
 for row in rows:
  path=row['source'];current=normalize(path,(R/path).read_bytes());assert sha(current)==row['current_sha256'],(label,path)
  text=current.decode()
  for e in row['edits']:
   assert text.count(e['new'])==1,(label,path,'unique inverse');text=text.replace(e['new'],e['old'],1)
  original=text.encode();assert sha(original)==row['original_sha256'];assert original==(D/'before'/label/path).read_bytes();assert original==git('show',B['refs'][label]+':'+path);count+=1
for row in B['protected_component_files']:
 assert sha((R/row['source']).read_bytes())==row['sha256'];assert (R/row['source']).read_bytes()==git('show',row['ref']+':'+row['source'])
for alias,path in B['main_aliases'].items():assert (R/alias).read_bytes()==git('show',B['refs']['main7c6']+':'+path)
# Original five/old Main9 and original compact table remain physical byte exact.
for path in ['docs/evidence/receiver-source-custody-v1/inverse.json','docs/evidence/main-b80-receiver-source-custody-20261016/inverse.json','docs/evidence/compact-staged-observations-v1/inverse.json']:
 assert (R/path).read_bytes()==git('show','0c942afa:'+path)
for row in json.loads((D/'new-owners.json').read_text()):
 assert sha((R/row['source']).read_bytes())==row['sha256'];assert len((R/row['source']).read_text().splitlines())==row['lines']<=1500
# Frozen90 files: streaming digest, never parse/transform the93MB corpus here.
fixtures=json.loads((R/B['fixture_inventory']).read_text());assert len(fixtures)==90;total=0
for f in fixtures:
 path=R/f['source'];assert path.stat().st_size==f['length'];total+=f['length']
 with path.open('rb')as stream:assert hashlib.file_digest(stream,'sha256').hexdigest()==f['sha256']
assert total==B['original90_bytes']==93641932
# Every fixed36 spec remains the reviewed one, including exact key multiplicity.
oldmeta=git('show',B['refs']['common']+':src/source_reflows_merge/metadata_json.rs').decode();meta=(R/'src/source_reflows_merge/metadata_json.rs').read_text();marker='    // Final fixed tables use the same authenticated reader; no payload embed.\n';assert marker in meta;assert meta[:meta.index(marker)]+'}\n'==oldmeta
# Unchanged pure projector and original private checked file-reader tail.
helper=(R/'src/source_reflows_merge.rs').read_text();oldhelper=git('show',B['refs']['common']+':src/source_reflows_merge.rs').decode()
def fn(text,anchor):
 i=text.index(anchor);return text[i:text.index('\n}',i)+2]
assert fn(helper,'fn project(rows:')==fn(oldhelper,'fn project(rows:')
oldruntime=git('show',B['refs']['common']+':'+C['selector_source']).decode();assert common.decode()[common.decode().index('fn read_fixture('):]==oldruntime[oldruntime.index('fn read_fixture('):]
# Both raw domains have named fronts before immutable predecessors; no serial
# arena->common or Main12->old5 rewrite is accepted.
assert helper.index('final_custody::input(final_custody::Fixture::Common',helper.index('fn main_dd0_previous'))<helper.index('metadata_json::previous',helper.index('fn main_dd0_previous'))
assert helper.index('final_custody::input(final_custody::Fixture::Arena',helper.index('fn arena854_input'))<helper.index('Fixture::ArenaReceiving',helper.index('fn arena854_input'))
assert 'final_custody::input(final_custody::Fixture::Main7c6' in helper
main=(R/'src/source_reflows/semantic_receivers_main7c6.rs').read_text();assert 'assert_eq!(proofs().len(), 12)'in main and 'complete compiled previous expression model'in main and 'restore_formatting(repair, &restored)'in main
# Replay all twelve exact Main inputs and all seven updated compact inputs.
for name in ['receiver-inverse12.json','compact-inverse7.json']:
 rows=json.loads((R/'docs/evidence/main7c6-hir-source-custody-20261016'/name).read_text())
 assert len(rows)==(12 if name.startswith('receiver')else 7)
 for row in rows:
  text=git('show',B['refs']['main7c6']+':'+row['path']).decode();assert sha(text.encode())==row['after_sha256']
  for e in row['inverse']:assert text.count(e['after'])==1;text=text.replace(e['after'],e['before'],1)
  assert sha(text.encode())==row['before_sha256'];base='receiver-source-custody-v1'if name.startswith('receiver')else'compact-staged-observations-v1';assert text==(R/'docs/evidence'/base/row['archive']).read_text()
# No changed original JSON or raised module budget is hidden by this packet.
for path in git('diff','--name-only','0c942afa','--','*.json').decode().splitlines():
 if git('ls-tree','0c942afa','--',path):assert (R/path).read_bytes()==git('show','0c942afa:'+path),path
assert (R/'tests/module-size-budget.tsv').read_bytes()==git('show','0c942afa:tests/module-size-budget.tsv')
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_input_rows':count,'scopes':B['scopes'],'protected_component_files':len(B['protected_component_files']),'original90_files':90,'original90_bytes':total,'fixed36_bytes':2612409,'Main12_and_Main7_complete_inverses':True,'rust_executed':False,'memory_benefit_measured':False},sort_keys=True))
