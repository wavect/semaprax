#!/usr/bin/env python3
"""Bounded standalone component check; no compiler, authority generation or execution."""
import argparse, hashlib, json, re
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);a=p.parse_args();R=Path(a.repo);D=R/'docs/evidence/main24-prefix-independent-next-20261016';N=R/'src/cleanup_plan/replay/sequence_prefix/main24'
def sha(b):return hashlib.sha256(b).hexdigest()
def function(s,name):
 start=s.index('fn '+name+'(');end=s.index('{',start);depth=1;i=end+1
 while depth:
  if s[i]=='{':depth+=1
  elif s[i]=='}':depth-=1
  i+=1
 return s[start:i]
b=json.loads((D/'source-bindings.json').read_text())
for name,h in b['new_owners'].items():assert sha((R/name).read_bytes())==h,name
for name,h in b['unchanged_main_blobs'].items():assert sha((N/name).read_bytes())==h,name
for name,h in b['tables'].items():assert sha((D/name).read_bytes())==h,name
assert sha((D/'main-helper.rs.txt').read_bytes())==b['main_helper_sha256']
for row in json.loads((D/'source-inverses.json').read_text()):
 current=(R/row['path']).read_text();assert sha(current.encode())==row['after_sha256']
 for e in row['inverse']:
  assert current.count(e['after'])==1
  current=current.replace(e['after'],e['before'],1)
 assert current==(D/row['archive']).read_text()
 assert sha(current.encode())==row['before_sha256']
assert function((N/'current.rs').read_text(),'sequence_skeleton_paths')==function((D/'main-replay.rs.txt').read_text(),'sequence_skeleton_paths')
assert function((N/'old_current.rs').read_text(),'sequence_skeleton_paths')==function((D/'main-f1-replay.rs.txt').read_text(),'sequence_skeleton_paths')
s=(N/'helper.rs').read_text()
s=s.replace('#[path = "before.rs"]\n','').replace('#[path = "wrapper_context.rs"]\n','').replace('#[path = "tests.rs"]\n','')
assert s==(D/'main-helper.rs.txt').read_text()
# Each literal module edge is explicit or preserved Main registration and physically resolves.
for name in ['mod.rs','helper.rs','wrapper_context.rs','before_wrapper.rs']:
 text=(N/name).read_text()
 for child in re.findall(r'#\[path\s*=\s*"([^"]+)"\]',text):assert (N/child).is_file(),(name,child)
assert 'use super::helper as sequence_prefix;' in (N/'current.rs').read_text()
assert 'use super::before_wrapper as sequence_prefix;' in (N/'old_current.rs').read_text()
assert 'use old_current::sequence_skeleton_paths;' in (N/'wrapper_context.rs').read_text()
assert 'use current::sequence_skeleton_paths;' in (N/'mod.rs').read_text()
checks=json.loads((D/'pending-selectors.json').read_text())['checks'];assert len(checks)==11
assert len(re.findall(r'#\[test\]',(N/'tests.rs').read_text()))==6
assert len(re.findall(r'#\[test\]',(N/'before_wrapper_tests.rs').read_text()))==5
print(json.dumps({'status':'SOURCE_ONLY_PASS','whole_existing_source_inverse':1,'exact_main_old_files':4,'complete_sequence_functions':2,'new_rust_children':len(b['new_owners']),'pending_controls':11,'shipping_change':'cfg(test) registration suffix only','source_alias_tables':'12 and 7 preserved exact; final raw-domain reader composition remains pending'},indent=2))
