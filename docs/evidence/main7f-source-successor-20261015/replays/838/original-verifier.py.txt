#!/usr/bin/env python3
"""Whole-source proof only. Does not compile or execute Rust."""
from pathlib import Path
import json,hashlib,subprocess,re,sys
root=Path(__file__).resolve().parents[3]
# Outboard candidate check: pass immutable checkout and candidate root explicitly.
if len(sys.argv)>1:root=Path(sys.argv[1])
candidate=Path(sys.argv[2]) if len(sys.argv)>2 else root
packet='docs/evidence/weighted-storage-838-main2b5-adapter-20261015';p=candidate/packet;sha=lambda b:hashlib.sha256(b).hexdigest();frozen='40f77262950e9dc4bb1da6626f441f2f504cfe59';helper='src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs'
def git(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=root)
held_rows=json.loads((root/'docs/evidence/held-auth-839/source-inverses.json').read_text())
held_projections=json.loads((root/'docs/evidence/source-composition-f813-held-20261015/held-projections.json').read_text())
def read(path):
 current=(candidate/path).read_bytes() if (candidate/path).exists() else (root/path).read_bytes()
 for r in held_projections:
  if r['source']==path:
   assert sha(current)==r['current_sha256'];assert current.decode()==r['edits'][0]['new'];current=r['edits'][0]['old'].encode();assert sha(current)==r['original_sha256']
 for r in held_rows:
  if r['path']==path:
   assert sha(current)==r['after_sha256'];assert current.decode()==r['after']
   if r['before'] is not None:assert sha(r['before'].encode())==r['before_sha256'];current=r['before'].encode()
 for r in json.loads((root/'docs/evidence/source-composition-f813-held-20261015/weighted-projections.json').read_text()):
  if r['source']==path:
   assert sha(current)==r['current_sha256'];assert current.decode()==r['edits'][0]['new'];current=r['edits'][0]['old'].encode();assert sha(current)==r['original_sha256']
 return current
def apply(source,patch,reverse):
 source=source.splitlines(keepends=True);output=[];at=0;inhunk=False
 for line in patch.splitlines(keepends=True):
  if line.startswith('@@ '):
   span=line.split()[2 if reverse else 1];start=int(span[1:].split(',')[0]);target=max(start-1,0);output.extend(source[at:target]);at=target;inhunk=True;continue
  if not inhunk:continue
  prefix=line[0];text=line[1:];remove=prefix==('+' if reverse else '-');add=prefix==('-' if reverse else '+')
  assert remove or add or prefix==' '
  if remove or prefix==' ':assert source[at]==text;at+=1
  if add or prefix==' ':output.append(text)
 output.extend(source[at:]);return ''.join(output)
rows=json.loads((p/'runtime-projections.json').read_text());assert len(rows)==4
for row in rows:
 current=read(row['source']);old=read(row['archive']);assert sha(current)==row['current_sha256'];assert sha(old)==row['original_sha256'];assert old==git(frozen,row['source'])
 for edit in row['edits']:assert current.decode().count(edit['new'])==1;current=current.decode().replace(edit['new'],edit['old'],1).encode()
 assert current==old
def current40f(binding):
 current=read(binding['source']).decode();matching=[r for r in rows if r['include_path']==binding['include_path']];assert len(matching)<=1
 if matching:
  for edit in matching[0]['edits']:assert current.count(edit['new'])==1;current=current.replace(edit['new'],edit['old'],1)
 assert current.encode()==git(frozen,binding['source']);return current
bindings=json.loads((p/'literal-bindings.json').read_text());assert len(bindings)==14
for b in bindings:
 current=current40f(b)
 if b['owner']=='source.rs':
  first=[v for v in bindings if v['owner']=='amortized_source.rs' and v['include_path']==b['include_path']]
  if b['include_path'] in ['../../canonical_shared.rs','../../../../../../tests/hir_module_boundaries.rs','../../../../../../tests/workspace/semantic_graph/source_inventory.rs']:
   assert len(first)==1;current=apply(current,read(first[0]['patch']).decode(),True)
 old=read(b['old']).decode();patch=read(b['patch']).decode();assert apply(old,patch,False)==current,b;assert apply(current,patch,True)==old,b
for row in json.loads((p/'immutable-patch-history.json').read_text()):assert sha(read(row['path']))==row['sha256'];assert read(row['path'])==git(frozen,row['path'])
row=json.loads((p/'source-inverses.json').read_text())[0];current=read(helper);old=read(row['archive']);assert sha(current)==row['current_sha256'];assert sha(old)==row['original_sha256'];assert old==git(row['checkpoint_revision'],helper);assert current.decode().replace(row['edits'][0]['new'],row['edits'][0]['old'],1).encode()==old
# Preserve the complete previous helper body, including both original assertions.
s=current.decode();start=s.index('\n// Complete current input is authenticated before either historical patch hop.');end=s.index('\n#[test]\n',start);s=s[:start]+s[end:];s=s.replace('    let projected = current40f(path, source);\n    let source = projected.as_str();\n','',1);s=s.replace('            let projected = current40f($current, include_str!($current));\n            let current = projected.as_str();','            let current = include_str!($current);',1);assert s==old.decode()
source='src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs';assert read(source)==git('2b5cae170e13b16c4a5f57a619b302405e2934a6',source)
for path in ['src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs','src/cleanup_plan/replay/factored/canonical_shared/before_scratch.rs','src/cleanup_plan/replay/factored/canonical_shared/before_scratch_memo.rs','src/cleanup_plan/replay/factored/canonical_shared/memo.rs','docs/evidence/canonical-scratch-839/canonical_shared.rs.before.txt','docs/evidence/canonical-scratch-839/memo.rs.before.txt']:
 assert read(path)==git(frozen,path),'unexpected839sourceguarddrift:'+path
print(json.dumps({'status':'SOURCE_ONLY_NO_RUST_EXECUTION','runtime_views':len(rows),'drift_sources':[r['source'] for r in rows],'amortized_bindings_forward_inverse':7,'original_bindings_forward_inverse':7,'old_patch_archive_files_preserved':len(json.loads((p/'immutable-patch-history.json').read_text())),'complete_previous_helper_preserved':True,'source_rs_untouched':True,'actual839_guard_input_drift':False},indent=2))
