#!/usr/bin/env python3
"""Verify only authenticated whole source bytes; never compile/execute Rust."""
from pathlib import Path
import hashlib,json,subprocess
root=Path(__file__).resolve().parents[3];packet=Path(__file__).resolve().parent;base='2e116e4f243e161dff5a8cbc335ed71418cf74d7';oldpacket=root/'docs/evidence/source36-current-view-adapter-94c392-20261015';sha=lambda b:hashlib.sha256(b).hexdigest();proofs=[]
for manifest in ['source-projections.json','source-inverses.json']:
 for row in json.loads((packet/manifest).read_text()):
  current=(root/row['source']).read_bytes();assert sha(current)==row['current_sha256'],row['source'];old=(root/row['archive']).read_bytes();assert sha(old)==row['original_sha256'],row['source']
  assert old==subprocess.check_output(['git','show',row['checkpoint_revision']+':'+row['source']],cwd=root)
  text=current.decode()
  for edit in row['edits']:
   assert text.count(edit['new'])==1,row['source'];text=text.replace(edit['new'],edit['old'],1)
  assert text.encode()==old,row['source'];proofs.append({'manifest':manifest,'source':row['source'],'whole_original_equal':True})
oldrecords=json.loads((oldpacket/'source-projections.json').read_text());newrecords=json.loads((packet/'source-projections.json').read_text());assert len(oldrecords)==2 and len(newrecords)==4 and newrecords[:2]==oldrecords
assert (oldpacket/'source-projections.json').read_bytes()==(packet/'metadata-history/old-two-view-source-projections.json').read_bytes()
a='src/cleanup_plan/replay/live_oracle/source_custody/a.rs';assert (root/a).read_bytes()==subprocess.check_output(['git','show',base+':'+a],cwd=root);assert len((root/a).read_text().splitlines())==1500
b='src/cleanup_plan/replay/live_oracle/source_custody/b.rs';text=(root/b).read_text()
for source,path in [('src/string_ops.rs','../../../../string_ops.rs'),('src/string_ops/conditions.rs','../../../../string_ops/conditions.rs')]:
 new=f'super::current_view::project("{source}", include_str!("{path}"))';old=f'include_str!("{path}").to_owned()';assert text.count(new)==1;text=text.replace(new,old,1)
assert text==(packet/'original'/b).read_text(),'all old undo/count/assertion bytes retained'
helper='src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs';new='/docs/evidence/source36-after-23e8-20261015/source-projections.json';old='/docs/evidence/source36-current-view-adapter-94c392-20261015/source-projections.json';text=(root/helper).read_text();assert text.count(new)==1;assert text.replace(new,old,1)==(packet/'original'/helper).read_text()
oldrunner=oldpacket/'verify_source36.py';expected=oldrunner.read_text().replace(old.lstrip('/'),new.lstrip('/'),1);assert expected==(packet/'verify_source36.py').read_text(),'only named combined-view JSON literal changes in adapted runner'
for path in ['docs/evidence/source36-current-view-adapter-94c392-20261015/verify_source36.py','docs/evidence/source36-current-view-adapter-94c392-20261015/verify_adapter.py','docs/evidence/source36-current-view-adapter-94c392-20261015/source-only-proof.json','docs/evidence/source36-current-view-adapter-94c392-20261015/source-projections.json','docs/evidence/source36-current-view-adapter-94c392-20261015/original/src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs','docs/evidence/source-projection-integration-797b/source-only-inverses.py']:
 assert (root/path).read_bytes()==subprocess.check_output(['git','show',base+':'+path],cwd=root),'old artifact changed: '+path
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 assert (root/path).read_text().count(helper)==1
assert all(not row['source'].endswith('.json') for row in json.loads((packet/'source-inverses.json').read_text()))
metadata=json.loads((packet/'metadata-history/local-metadata-inverse.json').read_text());current=(root/metadata['source']).read_bytes();assert sha(current)==metadata['current_sha256'];assert subprocess.run(['git','cat-file','-e',base+':'+metadata['source']],cwd=root,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode!=0
assert current.decode().replace(metadata['edits'][0]['new'],metadata['edits'][0]['old'],1).encode()==(root/metadata['archive']).read_bytes()==b''
print(json.dumps({'status':'SOURCE_ONLY_NO_RUST_EXECUTED','whole_source_proofs':proofs,'old_two_named_records_preserved':True,'old_runners_proofs_helper_archive_unchanged':True,'old_undo_count_assertions_preserved':True,'a_rs_unchanged_lines':1500,'new_json_not_global_active_source':True},indent=2))
