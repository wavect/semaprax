"""Finite source-only proof; no Cargo/runtime invocation or fixture updates."""
from pathlib import Path
import json,hashlib,subprocess,sys,ast,re
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='92eeddc7f9a6e65a849a1186642e0cc52a0a82e9';sha=lambda b:hashlib.sha256(b).hexdigest()
sys.path.insert(0,str(R/'docs/evidence/sourceguard-input-domains-7a87'));from source_inputs import Inputs
helper=Inputs(R);rows=json.loads((D/'current-to-92.json').read_text());assert len(rows)==3;neg=0
for row in rows:
 p=row['source'];raw=(R/p).read_text();old=helper.call('before_actual_pin_consumers',p,raw);assert old==(R/row['archive']).read_text()==subprocess.check_output(['git','show',BASE+':'+p],text=True)
 for bad in [raw+' ', ' '+raw]:
  try:helper.call('before_actual_pin_consumers',p,bad)
  except AssertionError:neg+=1
  else:raise AssertionError('changed complete input accepted')
 assert helper.call('before_sourceguard_inputs',p,raw)==subprocess.check_output(['git','show','d20af8ebf634b63e97fb84fbc5e9fe8242879720:'+p],text=True)
plan=json.loads((D/'actual-reviewed-plan/readonly-plan.json').read_text());record=json.loads((D/'actual-reviewed-plan/prepared-patch-record.json').read_text());assert len(plan['edits'])==11
for p in {e['path'] for e in plan['edits']}:
 old=subprocess.check_output(['git','show','7a87b33115c0cd45b14cac64d7b37e5742503159:'+p],text=True);new=old
 for e in plan['edits']:
  if e['path']==p:assert new.count(e['before'])==1;new=new.replace(e['before'],e['after'],1)
 assert new==(R/p).read_text()==(D/'actual-reviewed-plan/candidate-source-after'/p).read_text()
 assert sha(new.encode())==record['source_after_sha256'][p]
# Execute all original SourceA inverse assertions and all original frozen help
# transformations from the unchanged 92 verifier. Its immediate-row checks are
# separately composed above rather than supplied the wrong physical domain.
oldrunner=R/'docs/evidence/sourceguard-input-domains-7a87/verify.py';assert oldrunner.read_bytes()==subprocess.check_output(['git','show',BASE+':'+str(oldrunner.relative_to(R))]);code=oldrunner.read_text();section=code[code.index('# Execute the original unchanged'):code.index('# SourceB and the two historical')]
from help_literals import catalog,verify_frozen
scope=dict(a='src/cleanup_plan/replay/live_oracle/source_custody/a.rs',R=R,D=R/'docs/evidence/sourceguard-input-domains-7a87',BASE='d20af8ebf634b63e97fb84fbc5e9fe8242879720',helper=helper,re=re,json=json,ast=ast,subprocess=subprocess,catalog=catalog,verify_frozen=verify_frozen)
exec(compile(section,str(oldrunner),'exec'),scope);assert len(scope['sources'])==17 and len(scope['help_pins'])==11
for p in ['src/cleanup_plan/replay/live_oracle/source_custody/a.rs','src/cleanup_plan/replay/live_oracle/source_custody/b.rs','tests/offline_package/resolver_cli.rs','src/workspace_graph/identity_resolution/base/cache_billing.rs']:
 assert (R/p).read_bytes()==subprocess.check_output(['git','show',BASE+':'+p])
# Reuse the reviewed complete pin-source compositor and execute every original
# predicate against immutable checkpoint Git blobs, never alter the old table.
sys.path.insert(0,'/Volumes/Semaprax/provenance/pin-source-composition-v2-20261015');from source_inverse_composition import compose
candidates={p:(R/p).read_bytes() for p in {e['path'] for e in plan['edits']}}
projection,table,archives=compose(R,plan['capture_head'],'39130d9237121b61e40b47f95c072c09a456554e','docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json',candidates)
assert projection==json.loads((D/'actual-reviewed-plan/complete-source-projections.json').read_text());assert sum(r['legacy_inverse_count'] for r in projection['rows'])==107
assert table==(R/projection['legacy_table_path']).read_bytes()
result=dict(state='SOURCE_ONLY_NOT_RUNTIME_QUALIFICATION',checkpoint=BASE,actual_plan_literals=11,complete_frontdoor_rows=3,all17SourceA_original_full_inverses_equal=True,all11_original_help_pins_equal=True,legacy_full_source_inverse_predicates=107,tamper_refusals=neg,production_changes=0,new_test_bodies=0)
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
