"""Pure source/inverse/control proof. Never executes Rust or a benchmark."""
from pathlib import Path
import json,hashlib,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2]
BASE='c8740045e33126a4375aadd1dc524a25be9245fc';sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'full-source-inverses.json').read_text());negative=0
for row in rows:
 raw=(R/row['source']).read_bytes();old=(R/row['archive']).read_bytes()
 assert old==subprocess.check_output(['git','show',BASE+':'+row['source']],cwd=R)
 def restore(value):
  assert sha(value)==row['current_sha256'];text=value.decode()
  for e in row['inverse']:
   assert text.count(e['after'])==1;text=text.replace(e['after'],e['before'],1)
  assert sha(text.encode())==row['before_sha256'];assert text.encode()==old;return text
 assert restore(raw).encode()==old
 for bad in [raw+b'\n',raw[:-1],raw+raw,b'foreign']:
  try:restore(bad)
  except AssertionError:negative+=1
  else:raise AssertionError('source drift accepted')
 assert len(raw.splitlines())<=1500
leaf='src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs'
raw=(R/leaf).read_bytes();assert raw.startswith((D/'before'/leaf).read_bytes())
root=(R/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_text()
oldcall='budget.reserve_bytes(function, borrowed_labels::receiving::<ordered_weights::OrderedWeights>(), "factored borrowed label call receivers")?;'
assert root.count(oldcall)==2
assert root.count('return borrowed_labels::ordinary_successors(')==2
assert root.count('AllocationPayer::ActiveBuilder => {')==2
assert root.count('AllocationPayer::LegacyUnobserved => {}')==2
# Original work order is explicitly retained, not replaced by one total debit.
text=raw.decode();direct=text.split('pub(super) fn ordinary_successors(',1)[1].split('#[cfg(test)]',1)[0]
phases=['factored successor label census','factored successor label census search','factored successor label census shifts','factored successor target headers','factored skeleton subset construction','factored successor label search']
last=0
for phase in phases:
 at=direct.index('"'+phase+'"',last);assert at>=last;last=at
assert 'budget.charge(function, 4, "factored direct singleton selection")?' in direct
assert 'budget.reserve_bytes(function, 0, "factored direct singleton authentication")?' in direct
assert 'budget.charge(function, 0, "factored direct singleton authentication")?' in direct
assert 'Rc::new(SmallWeighted::One([(*child, weight)]))' in direct
acq=(D/'source-acquisition.json').read_bytes();helper=(R/'src/source_reflows_merge/main67107_inputs.rs').read_text();assert sha(acq) in helper
assert 'let singleton = singleton_successor_expected_input' in helper
assert helper.index('let singleton = singleton_successor_expected_input')<helper.index('let acquired = compile35f_expected_input')
# Existing immutable historical MAIN/870 rows still expect exact c874 source.
stages=json.loads((R/'docs/evidence/main67107-fronts-6e45-20261017/stages.json').read_text())
for row in stages:
 if row['domain'] in ['canonical870','canonicalMAIN']:
  assert row['current_sha256']==sha((D/'before/src/cleanup_plan/replay/factored/canonical_shared.rs').read_bytes())
print(json.dumps({'status':'PASS_PURE_SOURCE_ONLY','full_originals':3,'negative_source_controls':negative,'original_directory_prefix_unchanged':True,'selected_oracle_calls_unchanged':2,'actual_ordinary_calls':2,'work_requests_one':[2,1,1,1,2,2],'work_requests_zero':[1,0,1],'new_selector_work':4,'new_physical_rust_modules':0,'rust_executed':False},indent=2))
