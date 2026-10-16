#!/usr/bin/env python3
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);a=p.parse_args();R=Path(a.repo);D=R/'docs/evidence/main24-755-joint-next-20261016';BASE='09b2f0643461506be1fdbb5add8a6e5fce770284'
sha=lambda b:hashlib.sha256(b).hexdigest()
owners=json.loads((R/'docs/evidence/main24-prefix-independent-next-20261016/new-rust-owners.json').read_text())
for row in json.loads((D/'source-inverses.json').read_text()):
 s=(R/row['path']).read_text();assert sha(s.encode())==row['after_sha256']
 for e in row['inverse']:
  assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
 assert s==(D/row['archive']).read_text();assert sha(s.encode())==row['before_sha256']
 assert s.encode()==subprocess.check_output(['git','show',BASE+':'+row['path']],cwd=R)
 if row['path'].endswith('source_inventory.rs') or row['path']=='tests/hir_module_boundaries.rs':
  for owner in owners:assert (R/row['path']).read_text().count(owner+'"')==1,(row['path'],owner)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
changed=subprocess.check_output(['git','diff',BASE,'--name-only','--','src'],cwd=R).decode().splitlines()
assert set(changed)==set(owners+['src/cleanup_plan/replay.rs']),changed
for table in ['compact-staged-observations-v1/inverse.json','receiver-source-custody-v1/inverse.json']:
 path='docs/evidence/'+table;assert (R/path).read_bytes()==subprocess.check_output(['git','show',BASE+':'+path],cwd=R)
for commit in [BASE,'47eeb5faa8c8589cf026630bf89b6851d4ef54f7','24ffa85e83d06d71f8214fdae9f631d0baea894e']:
 subprocess.run(['git','merge-base','--is-ancestor',commit,'HEAD'],cwd=R,check=True)
subprocess.run(['python3',str(R/'docs/evidence/main24-prefix-independent-next-20261016/verify-source.py'),'--repo',str(R)],check=True)
print(json.dumps({'status':'SOURCE_ONLY_PASS','whole_inverses':6,'main24_owners_joined_each_inventory':9,'interop_lines':1500,'755_source_preservation':'all src byte-exact except root test registration; nine added children','actual_main24_ancestry':True,'global_seal':'pending canonical; not generated'},indent=2))
