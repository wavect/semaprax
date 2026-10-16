#!/usr/bin/env python3
"""Finite source preservation only; does not execute or qualify Rust behavior."""
from pathlib import Path
import hashlib,json,re,copy
E=Path(__file__).resolve().parent;R=E.parents[2]
M=json.loads((E/'predecessor-views.json').read_text())
def sha(data):return hashlib.sha256(data).hexdigest()
def restore(row,current):
 assert sha(current)==row['current_sha256'],row['path']
 lines=current.decode().splitlines(keepends=True)
 for edit in reversed(row['edits']):
  a,z=edit['current_start'],edit['current_end']
  assert ''.join(lines[a:z])==edit['current']
  lines[a:z]=edit['original'].splitlines(keepends=True)
 old=''.join(lines).encode()
 assert sha(old)==row['sha256']
 assert old==(E/row['archive']).read_bytes()
 return old
original={}
for row in M['rows']:
 original[row['path']]=restore(row,(R/row['path']).read_bytes())
for row in M['new_sources']+M['unchanged_owners']:
 assert sha((R/row['path']).read_bytes())==row['sha256'],row['path']
C='src/cleanup_plan/replay/factored/canonical_shared'
assert (R/(C+'/before_terminal_leaf_empty_memo.rs')).read_bytes()==original[C+'/empty_memo.rs']
old=original[C+'.rs'].decode();start=old.index('// Invoked only after preflight');stop=old.index('\n#[cfg(test)]\nmod tests;')
body=old[start:stop]
frozen=(R/(C+'/before_terminal_leaf.rs')).read_text()
assert frozen.count(body)==1
# Full general producer including all failure exits is unchanged.
current=(R/(C+'.rs')).read_text();marker='    pub(super) fn id_with_receiving('
assert current[current.index(marker):current.index('\n#[cfg(test)]\nmod tests;')]==old[old.index(marker):stop]
for p in ['before_arena.rs','before_empty_signature.rs']:
 current=(R/(C+'/'+p)).read_text()
 assert current.replace('empty_closure, before_terminal_leaf_empty_memo as empty_memo,','empty_closure, empty_memo,')==original[C+'/'+p].decode()
base=json.loads((E/'base.json').read_text())
for p in base['raw_guards']:
 text=(R/p).read_text()
 for new in base['new_rust_sources']:
  assert len(re.findall(r'include_str!\("[^"\n]*'+re.escape(new)+r'"\)',text))==1,(p,new)
 assert text.count('\n')<=1500 if p.startswith('crates/') else True
for p in base['new_rust_sources']+[C+'.rs',C+'/empty_memo.rs']:
 assert len((R/p).read_text().splitlines())<=1500,p
root=(R/(C+'.rs')).read_text()
for module in ['terminal_leaf_tests','before_terminal_leaf','before_terminal_leaf_empty_memo']:
 assert '#[cfg(test)]\nmod '+module+';' in root
assert '\nmod terminal_leaf;' in root
# Existing named old assertion counts remain complete, not a selector-only claim.
for row in M['unchanged_owners']:
 assert len(re.findall(rb'\bassert(?:_eq|_ne)?!',(R/row['path']).read_bytes()))==row['assertions']
controls=0
for row in M['rows']:
 data=(R/row['path']).read_bytes()
 for bad in [data+b' ',data[:-1],data.replace(b'include_str!',b'include_xtr!',1) if b'include_str!' in data else data[:1]+b'!'+data[2:]]:
  if bad==data:continue
  try:restore(row,bad)
  except AssertionError:controls+=1
  else:raise AssertionError('drift accepted')
 # Even a forged endpoint cannot make a body change restore the complete original.
 if row['edits']:
  bad=copy.deepcopy(row);bad['edits'][0]['original']+=' '
  try:restore(bad,data)
  except AssertionError:controls+=1
  else:raise AssertionError('forged inverse accepted')
print(json.dumps({'status':'PASS_SOURCE_ONLY','complete_preimages':len(M['rows']),'full_new_sources':len(M['new_sources']),'unchanged_owner_files':len(M['unchanged_owners']),'unchanged_assertions':sum(x['assertions'] for x in M['unchanged_owners']),'rejected_drift_controls':controls,'rust_execution':'NOT_RUN'},indent=2))
