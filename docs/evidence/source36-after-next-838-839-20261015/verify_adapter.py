#!/usr/bin/env python3
"""Source-only full-byte proof; never compile or execute Rust."""
from pathlib import Path
import json,hashlib,subprocess
root=Path(__file__).resolve().parents[3];packet=Path(__file__).resolve().parent;base='5f1fecd42694c52fbdcec67f9ef3d3fd0b7d7f3b';sha=lambda b:hashlib.sha256(b).hexdigest()
oldpacket=root/'docs/evidence/source36-after-23e8-20261015'
old=json.loads((oldpacket/'source-projections.json').read_text());new=json.loads((packet/'source-projections.json').read_text());assert len(old)==len(new)==4
for prior,row in zip(old,new):
 if row['source']!='tests/hir_module_boundaries.rs':assert prior==row
 else:
  for key in ['source','archive','checkpoint_revision','original_sha256']:assert prior[key]==row[key]
 current=(root/row['source']).read_bytes();original=(root/row['archive']).read_bytes();assert sha(current)==row['current_sha256'];assert sha(original)==row['original_sha256'];assert original==subprocess.check_output(['git','show',row['checkpoint_revision']+':'+row['source']],cwd=root)
 text=current.decode()
 for edit in row['edits']:assert text.count(edit['new'])==1;text=text.replace(edit['new'],edit['old'],1)
 assert text.encode()==original
for row in json.loads((packet/'source-inverses.json').read_text()):
 current=(root/row['source']).read_bytes();original=(root/row['archive']).read_bytes();assert sha(current)==row['current_sha256'];assert sha(original)==row['original_sha256'];assert original==subprocess.check_output(['git','show',base+':'+row['source']],cwd=root)
 assert current.decode().replace(row['edits'][0]['new'],row['edits'][0]['old'],1).encode()==original
 oldliteral='/docs/evidence/source36-after-23e8-20261015/source-projections.json';newliteral='/docs/evidence/source36-after-next-838-839-20261015/source-projections.json';assert current.decode().replace(newliteral,oldliteral,1).encode()==original
for row in json.loads((packet/'prior-artifact-hashes.json').read_text()):
 data=(root/row['path']).read_bytes();assert sha(data)==row['sha256'];assert data==subprocess.check_output(['git','show',base+':'+row['path']],cwd=root)
for stem in ['a','b']:
 path=f'src/cleanup_plan/replay/live_oracle/source_custody/{stem}.rs';data=(root/path).read_bytes();assert data==subprocess.check_output(['git','show',base+':'+path],cwd=root)
 if stem=='a':assert len(data.splitlines())==1500
prior=(oldpacket/'verify_source36.py').read_text();assert prior.replace(str(oldpacket.relative_to(root)/'source-projections.json'),str(packet.relative_to(root)/'source-projections.json'),1)==(packet/'verify_source36.py').read_text()
metadata=json.loads((packet/'metadata-history/local-metadata-inverse.json').read_text());data=(root/metadata['source']).read_bytes();assert sha(data)==metadata['current_sha256'];assert data.decode().replace(metadata['edits'][0]['new'],'',1).encode()==(root/metadata['archive']).read_bytes()==b''
assert (packet/'metadata-history/old-four-view-source-projections.json').read_bytes()==(oldpacket/'source-projections.json').read_bytes()
print(json.dumps({'status':'SOURCE_ONLY_NO_RUST_EXECUTED','named_views':4,'changed_view':'tests/hir_module_boundaries.rs','unchanged_prior_views':3,'active_source_inverses':1,'a_rs_unchanged_lines':1500,'b_rs_unchanged':True,'old_artifacts_preserved':len(json.loads((packet/'prior-artifact-hashes.json').read_text())),'original_runner_only_manifest_literal_adapted':True},indent=2))
