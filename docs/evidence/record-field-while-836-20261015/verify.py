#!/usr/bin/env python3
"""Bounded source proof only; no Rust execution or backend acceptance."""
import hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest()
def project(rows,path,current):
 matching=[r for r in rows if r['source']==path];assert len(matching)<=1
 if not matching:return current
 r=matching[0];assert h(current.encode())==r['current_sha256'],path
 for e in r['edits']:
  assert current.count(e['new'])==1,path
  current=current.replace(e['new'],e['old'],1)
 assert h(current.encode())==r['original_sha256'],path
 assert current.encode()==(ROOT/r['archive']).read_bytes(),path
 return current
rows=json.loads((HERE/'current-to-7a87.json').read_text())
assert rows==json.loads((HERE/'source-inverses.json').read_text())
for r in rows:
 original=project(rows,r['source'],(ROOT/r['source']).read_text())
 assert original.encode()==subprocess.check_output(['git','show',r['checkpoint_revision']+':'+r['source']],cwd=ROOT)
views=json.loads((HERE/'backend-input-views.json').read_text());count=0
for domain,packet in [('backends','record-copy-field-update-836-backends'),('coverage','record-copy-field-update-836-backend-controls')]:
 data=json.loads((ROOT/'docs/evidence'/packet/'source-inverses.json').read_text())
 # Replay every actual original owner edit/count/final comparison, then every child SHA.
 for row in data['owners']:
  text=project(views[domain],row['source'],(ROOT/row['source']).read_text())
  assert h(text.encode())==row['current_sha256'],row['source']
  for e in row['edits']:
   assert text.count(e['new'])==1,(domain,row['source'])
   text=text.replace(e['new'],e['before'],1)
  original=(ROOT/row['archive']).read_bytes();assert text.encode()==original
  assert h(original)==row['original_sha256'];count+=1
 for row in data['children']:
  text=project(views[domain],row['source'],(ROOT/row['source']).read_text())
  assert h(text.encode())==row['sha256'],(domain,row['source']);count+=1
support=(ROOT/'tests/support/record_copy_field_update.rs').read_text()
assert support.count('current = source_input(')==2
assert support.count('let bytes = source_input(')==2
for name in ['src/hir/resolve_statement.rs','src/source_verify/iterative/while_rules.rs','src/source_verify/oracle/while_oracle.rs']:
 text=(ROOT/name).read_text();assert text.count('// The source-only literal selector is removed before HIR.')==1
 assert 'args.len()' in text
# Complete independent historical bodies remain exact.
for path in ['src/hir/borrowed_root_oracle/resolve_statement.rs','src/source_verify/iterative/while_rules/before_nested.rs','src/vec_ops.rs','src/hir/vec_loop_renewal.rs']:
 assert (ROOT/path).read_bytes()==subprocess.check_output(['git','show','7a87b33115c0cd45b14cac64d7b37e5742503159:'+path],cwd=ROOT)
print(json.dumps({'status':'SOURCE_ONLY_PASS','immediate_full_inverses':len(rows),'backend_full_input_checks':count,'changed_backend_views':{k:len(v) for k,v in views.items()},'rust_execution':False},indent=2))
