#!/usr/bin/env python3
"""Authenticate complete immediate originals and the two exact checkpoint views."""
from pathlib import Path
import hashlib,json,subprocess
root=Path(__file__).resolve().parents[3]
packet=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
results=[]
for name in ['source-projections.json','source-inverses.json']:
 for row in json.loads((packet/name).read_text()):
  source=row['source'];current=(root/source).read_bytes();assert sha(current)==row['current_sha256'],source
  old=(root/row['archive']).read_bytes();assert sha(old)==row['original_sha256'],source
  prior=subprocess.run(['git','show',row['checkpoint_revision']+':'+source],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  assert prior.stdout==old and (prior.returncode==0 or old==b''),source
  projected=current.decode()
  for edit in row['edits']:
   assert projected.count(edit['new'])==1,source
   projected=projected.replace(edit['new'],edit['old'],1)
  assert projected.encode()==old,source
  results.append(dict(manifest=name,source=source,complete_original_equal=True))
a='src/cleanup_plan/replay/live_oracle/source_custody/a.rs'
b=a.replace('/a.rs','/b.rs')
for source,old,new in [(a,'include_str!("../../../replay.rs").to_owned()','super::current_view::project("src/cleanup_plan/replay.rs", include_str!("../../../replay.rs"))'),(b,'include_str!("../../../../../tests/hir_module_boundaries.rs").to_owned()','super::current_view::project("tests/hir_module_boundaries.rs", include_str!("../../../../../tests/hir_module_boundaries.rs"))')]:
 current=(root/source).read_text();original=(packet/'original'/source).read_text()
 assert current.count(new)==1
 assert current.replace(new,old,1)==original,'all original undo/assertion/body bytes retained'
assert len((root/a).read_text().splitlines())==1500
helper='src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs'
for source in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 assert (root/source).read_text().count(helper)==1
old='docs/evidence/source-projection-integration-797b/source-only-inverses.py'
assert (root/old).read_bytes()==subprocess.check_output(['git','show','94c392e9bd8b60edba497263d0267ec54a4dd728:'+old],cwd=root)
# Every changed Rust source is test-only; all production files stay at the source freeze.
changed=subprocess.check_output(['git','diff','--name-only','94c392e9bd8b60edba497263d0267ec54a4dd728','--','*.rs'],cwd=root).decode().splitlines()
allowed={row['source'] for row in json.loads((packet/'source-inverses.json').read_text())}
assert set(changed)<=allowed
print(json.dumps(dict(schema='source36-current-view-adapter.v1',execution='source-only; no Rust executed',whole_source_identities=results,all_original_undo_and_assertion_bytes_preserved=True,source_a_lines=1500,old_source36_runner_unchanged=True),indent=2))
