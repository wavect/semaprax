"""Bounded source-only proof for actual compiler diagnostic fixes; no Rust runs."""
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='7827d3687da21e1093d40f8c8e3f7116edba1993';sha=lambda b:hashlib.sha256(b).hexdigest();git=lambda p:subprocess.check_output(['git','show',BASE+':'+p],cwd=R)
rows=json.loads((D/'whole-inverses.json').read_bytes());assert len(rows)==2
for row in rows:
 before=(D/row['before_archive']).read_bytes();after=(R/row['path']).read_bytes();assert before==git(row['path']) and sha(before)==row['before_sha256'] and sha(after)==row['after_sha256'];assert row['inverse']==[dict(after=after.decode(),before=before.decode())]
p=rows[0]['path'];old=git(p).decode();new=(R/p).read_text();assert new.splitlines(True)[:204]==old.splitlines(True)[:204]
substitutions=[('plan_skeleton_paths(function, &mut current)','super::super::plan_skeleton_paths(function, &mut current)'),('plan_skeleton_paths(function,&mut current)','super::super::plan_skeleton_paths(function,&mut current)'),('plan_skeleton_paths(function,&mut budget)','super::super::plan_skeleton_paths(function,&mut budget)')]
restored=new
for before,after in substitutions:assert restored.count(after)==1;restored=restored.replace(after,before,1)
assert restored==old
# Complete old reference calls, full cross-generation comparator, work/materialization
# assertions, production probe counters, six Diagnostic fields and Drop remain intact.
for literal in ['with_original(|| compact_stage::original_paths(function, &mut old))','with_original(||compact_stage::original_paths(function,&mut old))','compact_stage::paths_match(&actual, &expected)','compact_stage::paths_match(&actual,&expected)','GENERIC_TAKES.with(Cell::get)>takes','fn probe_failure(error: &Diagnostic)','pub(super) fn original_selected()']:
 assert literal in new
root=(R/'src/cleanup_plan/replay.rs').read_text();assert re.search(r'fn plan_skeleton_paths\([\s\S]*?\) -> Result<Vec<SkeletonPath>, Diagnostic>',root)
compact=(R/'src/cleanup_plan/replay/compact_stage.rs').read_text();assert 'fn paths_match(actual: &[SkeletonPath], expected: &[before::SkeletonPath]) -> bool' in compact
branch=(R/'src/cleanup_plan/replay/branch_clone.rs').read_text();assert 'use before::{main_replay_input, plan_skeleton_paths};' in branch and 'mod tests;' in branch
oldtypes=(R/'src/cleanup_plan/replay/branch_clone/before.rs').read_text();assert 'compact_stage::before::{SkeletonObservation, SkeletonPath, lexical_skeleton}' in oldtypes
p=rows[1]['path'];old=git(p).decode();new=(R/p).read_text();assert new.replace('.and_then(|_reserved_slots| produce(function, &mut budget))','.and_then(|()| produce(function, &mut budget))',1)==old
assert re.search(r'fn reserve_program_skeleton_work[^\{]+-> Result<usize, Diagnostic>',root)
physical={r['path'] for r in json.loads((R/'docs/evidence/main09013-source-namespaces-v1/input-domains.json').read_bytes())['physical']};plan={r['path'] for r in json.loads((R/'docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json').read_bytes())};changed={r['path'] for r in rows};assert changed&physical=={'src/cleanup_plan/replay/branch_clone/tests.rs'} and not changed&plan
for row in rows:assert len((R/row['path']).read_text().splitlines())<=1500
errors=Path('/Volumes/Semaprax/provenance/final443-first-compilation-errors-7827-20261017.json')
result=dict(status='PASS_SOURCE_ONLY_FIX_NOT_RECOMPILED',base=BASE,compiler_error_packet=dict(path=str(errors),sha256=sha(errors.read_bytes())),whole_before_inverse_records=2,production_root_calls_explicitly_selected=3,complete_original_test_prefix_lines_unchanged=204,complete_cross_type_comparator_unchanged=True,independent_old_producer_calls_unchanged=True,all_work_materialization_Diagnostic_refusal_floor_and_Drop_controls_unchanged=True,PLAN_reserved_slots_Result_usize_received=True,MAIN090_physical_intersection=sorted(changed&physical),PLAN6_intersection=[],source_custody_full_rebind_executed=False,Cargo_rustc_build_or_runtime_executed=False)
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
