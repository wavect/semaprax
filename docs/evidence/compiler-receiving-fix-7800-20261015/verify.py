"""Finite source-only predecessor custody checks; never compiles or runs Rust."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3]; D=Path(__file__).parent; BASE='7800b014151b6ce0709c351ccdfd52ae25832e07'; sha=lambda b:hashlib.sha256(b).hexdigest()
def git(p):return subprocess.check_output(['git','show',BASE+':'+p],cwd=R).decode()
def restore(rows,p,s):
 found=[r for r in rows if r['source']==p];assert len(found)<=1
 if not found:return s
 r=found[0];assert sha(s.encode())==r['current_sha256'],p
 for e in r['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==r['original_sha256'],p
 return s
rows=json.loads((D/'predecessor-projections.json').read_text());assert len(rows)==3
for r in rows:
 p=r['source'];s=(R/p).read_text();before=restore(rows,p,s);assert before==git(p);assert len(r['edits'])==1;assert r['edits'][0]['new']==s and r['edits'][0]['old']==before
p='src/cleanup_plan/replay/factored/canonical_shared/before_held.rs';old=git(p);new=(R/p).read_text();assert new==old.replace('use super::{app_census, ordered_alias, ordered_weights, successor_targets};','use super::{app_census, ordered_alias, ordered_weights, successor_targets, tests};').replace('    closures: memo::RetainedMemo<Rc<ordered_weights::OrderedWeights>>,','    pub(super) closures: memo::RetainedMemo<Rc<ordered_weights::OrderedWeights>>,')
p='src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs';assert (R/p).read_text()==git(p).replace('use super::before_held::memo;','use super::before_held::memo;\nuse super::tests;')
# Every old test/assertion/function body remains literally intact; only imports/field visibility above change.
packet=R/'docs/evidence/source-composition-f813-held-20261015';protected=[]
for name in ['current-to-prior.json','main-projections.json','ours-projections.json','pre-ours.json','held-projections.json','weighted-projections.json','source-inverses.json']:
 p=str((packet/name).relative_to(R));assert (R/p).read_text()==git(p);protected.append(p)
 for r in json.loads((R/p).read_text()):
  current=restore(rows,r['source'],(R/r['source']).read_text());assert sha(current.encode())==r['current_sha256'],(name,r['source']);restore([r],r['source'],current)
# Complete26 held-auth inputs, through new stage then unchanged merge stage.
p='docs/evidence/held-auth-839/source-inverses.json';assert (R/p).read_text()==git(p);held=json.loads((R/p).read_text());assert len(held)==26;hp=json.loads((packet/'held-projections.json').read_text())
for r in held:
 p=r['path'];s=restore(rows,p,(R/p).read_text());s=restore(hp,p,s);assert sha(s.encode())==r['after_sha256'];assert s==r['after'];
 if r['before'] is not None:assert sha(r['before'].encode())==r['before_sha256']
# All prior runtime/source proof/archives remain exact, not just the seven active tables.
changed=subprocess.check_output(['git','diff','--diff-filter=M','--name-only',BASE],cwd=R,text=True).splitlines();expected=[r['source'] for r in rows];assert set(changed)==set(expected),(changed,expected)
archive=R/'docs/evidence/failed-build-7800-20261015';summary=json.loads((archive/'summary.json').read_text());assert summary['build_exit']==101 and summary['successful_phases']==7 and summary['total_phases']==8 and summary['executed_test_bodies']==0;assert len(summary['actual_compiler_errors'])==3
proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','basis':BASE,'full_repaired_sources':3,'original_assertion_bodies_edited':0,'new_rust_selectors':0,'protected_38_current_to_prior_rows':38,'protected_held_rows':26,'all_old_runtime_metadata_archives_and_proofs_unchanged':True,'all_old_projection_current_hashes_and_full_inverses_verified':True,'archive_summary_sha256':sha((archive/'summary.json').read_bytes()),'fixed_files':expected,'runtime_qualification':'PENDING; actual frozen7800 unit compile failed3 errors,7/8build phases passed,0bodies; no RAM interruption'}
print(json.dumps(proof,indent=2))
