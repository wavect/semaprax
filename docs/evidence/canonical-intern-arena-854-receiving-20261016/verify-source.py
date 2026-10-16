#!/usr/bin/env python3
"""Finite receiving successor, preserving every earlier whole-source capsule."""
import argparse,json,hashlib,pathlib,subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);a=ap.parse_args();r=pathlib.Path(a.repo).resolve();p=r/'docs/evidence/canonical-intern-arena-854-receiving-20261016';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(h,s):return subprocess.check_output(['git','show',h+':'+s],cwd=r)
physical={}
def rewind(packet,table,base,count):
 rows=json.loads((packet/table).read_text());assert len(rows)==count
 for row in rows:
  s=physical.get(row['source'],(r/row['source']).read_text());assert sha(s.encode())==row['current_sha256']
  for e in row['edits']:
   assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
  assert sha(s.encode())==row['original_sha256'];assert s.encode()==git(base,row['source']);assert s.encode()==(r/row['original_archive']).read_bytes();physical[row['source']]=s
rewind(p,'current-to-9db.json','9db046a8673ae083f5aa0303fb369e1357463614',7)
joined=r/'docs/evidence/canonical-intern-arena-854-joined-20261016';component=r/'docs/evidence/canonical-intern-arena-854-next-20261016'
for packet in [joined,component]:
 paths=subprocess.check_output(['git','ls-tree','-r','--name-only','9db046a8673ae083f5aa0303fb369e1357463614',str(packet.relative_to(r))],cwd=r,text=True).splitlines()
 for source in paths:assert (r/source).read_bytes()==git('9db046a8673ae083f5aa0303fb369e1357463614',source)
rewind(joined,'current-to-ab13.json','ab13bc4855ab9559e2e18725c4ce1147a5071bd0',8)
rewind(component,'current-to-3c388.json','3c3886620dff92f0d5ae8f9e5a7b097e3c3c73ad',3)
for row in json.loads((p/'new-owners.json').read_text()):
 b=(r/row['source']).read_bytes();assert sha(b)==row['sha256'];assert len(b.decode().splitlines())==row['lines']<=1500
root=(r/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_text();arena=(r/'src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs').read_text()
for kind in ['SharedWeighted','intern_arena::Signature']:
 assert '2 * std::mem::size_of::<Vec<intern_arena::Node<'+kind+'>>>()'in root
assert '2 * size_of::<&Vec<usize>>()'in arena
assert arena.count('2 * size_of::<&Vec<(Label, SharedWeighted)>>()')==2
# Shipping algorithms, both physical/work owners and the complete old receiver
# remain exact; only declared constructor/borrow prices differ in production.
for source in ['src/cleanup_plan/replay/factored.rs','src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs','src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs']:
 assert (r/source).read_bytes()==git('9db046a8673ae083f5aa0303fb369e1357463614',source)
assert len(json.loads((p/'pending-selectors.json').read_text()))==3
print(json.dumps({'status':'PASS_RECEIVING_SUCCESSOR_SOURCE_ONLY','new_whole_rows':7,'protected_join_rows':8,'protected_component_rows':3,'protected_packets_byte_exact':True,'complete_old_receiver_and_physical_controls_exact':True,'new_test_bodies':0,'rust_execution':False,'gain_claim':False},indent=2))
