"""Pure full-source/inverse and exact old-stage composition controls."""
from pathlib import Path
import hashlib,json,subprocess,importlib.util
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'full-source-inverses.json').read_text());neg=0
for row in rows:
 raw=(R/row['source']).read_bytes();old=(R/row['archive']).read_bytes()
 head='809eee10c' if row['source'].endswith('main67107_inputs.rs') else 'c8740045e'
 assert old==subprocess.check_output(['git','show',head+':'+row['source']],cwd=R)
 def restore(value):
  assert sha(value)==row['current_sha256'];s=value.decode()
  for e in row['inverse']:
   assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
  assert s.encode()==old and sha(old)==row['before_sha256']
 restore(raw)
 for bad in [raw+b'\n',raw[:-1],raw+raw,b'foreign']:
  try:restore(bad)
  except AssertionError:neg+=1
  else:raise AssertionError('foreign source accepted')
 assert len(raw.splitlines())<=1500
spec=importlib.util.spec_from_file_location('closed_fixture_common',D/'run-common-corrected-fixtures.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
oldstage=(D/'stage.before.py.txt').read_bytes();assert oldstage==(R/'docs/evidence/compile-acquisition-35f-20261017/stage.py').read_bytes()
runner=(D/'run-common.before.py.txt').read_bytes();assert runner==(R/'docs/evidence/compile-acquisition-35f-20261017/run-common.py').read_bytes()
original=m.render_stage(oldstage) # Definitions only; no generator or main call.
row=next(r for r in json.loads((D/'acquisition.json').read_text())if r['source'].endswith('rename_tests.rs'))
raw=(R/row['source']).read_bytes();before=(R/row['archive']).read_bytes()
oldrow=next(r for r in json.loads((R/'docs/evidence/compile-acquisition-35f-20261017/sources.json').read_bytes())if r['source']==row['source'])
old=(R/oldrow['before_archive']).read_bytes()
for stage in [raw,before,old]:assert original.stage(R,row['source'],stage)==old
for bad in [raw+b'\n',raw[:-1],raw+raw,b'foreign']:
 try:original.stage(R,row['source'],bad)
 except AssertionError:neg+=1
 else:raise AssertionError('foreign old-stage input accepted')
assert original.stage(R,'unknown','unchanged')=='unchanged'
text=(R/rows[0]['source']).read_text();assert 'crate::format::canonical(' in text and 'source.as_str()' in text
text=(R/rows[1]['source']).read_text();assert 'fn identity<T>(value:T)->T' in text
print(json.dumps(dict(status='PASS_PURE_SOURCE_ONLY',whole_originals=3,hostile_source_and_stage_refusals=neg,complete_stages=['actualnew','c874','35f'],all_old_scripts_unchanged=True,no_generators_or_rust_executed=True),indent=2))
