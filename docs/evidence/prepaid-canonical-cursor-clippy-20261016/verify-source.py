#!/usr/bin/env python3
"""One production attribute, complete predecessor input, unchanged old algorithms."""
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);R=A.parse_args().repo.resolve();D=R/'docs/evidence/prepaid-canonical-cursor-clippy-20261016';BASE='f7666352f59ba0569b8dad769c162fbf0b978b22';OLD=R/'docs/evidence/release-755-main24-arena-joined-20261016';sha=lambda b:hashlib.sha256(b).hexdigest();CO=subprocess.check_output
rows=json.loads((D/'current-to-f766.json').read_text());assert len(rows)==4;view={}
for row in rows:
 path=row['source'];b=(R/path).read_bytes();assert sha(b)==row['current_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 b=s.encode();assert sha(b)==row['original_sha256'];assert b==CO(['git','show',BASE+':'+path],cwd=R)==(D/'before/f766'/path).read_bytes();view[str((R/path).resolve())]=b
path='src/cleanup_plan/replay/factored/canonical_shared.rs';current=(R/path).read_text();old=view[str((R/path).resolve())].decode();attribute='                            #[expect(\n                                clippy::explicit_counter_loop,\n                                reason = "The scalar cursor is prepaid above; enumerate would add an iterator-adapter receiving layout."\n                            )]\n';assert current.count(attribute)==1;assert current.replace(attribute,'',1)==old
assert '2 * std::mem::size_of::<usize>()'in old and 'canonical signature miss indexing'in old
for p in CO(['git','ls-tree','-r','--name-only',BASE,'--',str(OLD.relative_to(R))],cwd=R).decode().splitlines():assert(R/p).read_bytes()==CO(['git','show',BASE+':'+p],cwd=R),(p,'immutable prior packet')
rb0,rt0,wt0=Path.read_bytes,Path.read_text,Path.write_text;argv=sys.argv[:]
def rb(p):return view.get(str(p.resolve()),rb0(p))
def rt(p,*args,**kw):
 b=view.get(str(p.resolve()));return b.decode(kw.get('encoding')or'utf-8')if b is not None else rt0(p,*args,**kw)
def wt(p,s,*args,**kw):assert p.resolve().is_relative_to(R/'docs/evidence');return len(s)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;sys.argv=[str(OLD/'verify-source.py'),'--repo',str(R)];out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(OLD/'verify-source.py'),run_name='__main__')
 prior=json.loads(out.getvalue())
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;sys.argv=argv
source=(R/'src/source_reflows_merge.rs').read_text()
for name in ['main_dd0_previous','arena854_input','main7c6_input','main_afba_input','main_f1_input','main24_input']:
 block=source[source.index('fn '+name+'('):];assert block.index('cursor_clippy_input')<block.index('release_custody::input')
phase=D/'failed-f766-core-phase';files=json.loads((D/'phase-files.json').read_text())
for p,row in files.items():b=(phase/p).read_bytes();assert len(b)==row['bytes'] and sha(b)==row['sha256']
result=json.loads((phase/'clippy-production-result.json').read_text());assert result['source_subject']==BASE and result['exit_code']==101 and result['executed_test_bodies']==0;assert result['resource_refusal']is None and not result['observation_errors'];assert result['group_settlement']['confirmed_empty'] and result['post_stop_observation_status']=='collected_after_confirmed_empty_group';assert result['maximum_positive_swap_used_delta_bytes']==0
assert sha((phase/'clippy-production.log').read_bytes())==result['log_sha256'];assert sha((phase/'clippy-production-memory-samples.jsonl').read_bytes())==result['memory_samples_sha256'];assert sha((phase/'drivers/run-production-lib-cli-clippy.py').read_bytes())==result['driver_sha256'];assert sha((phase/'drivers/semaprax_pressure_observation_v3.py').read_bytes())==result['observation_helper_sha256'];assert sha((phase/'drivers/observed_phase.py').read_bytes())==result['phase_monitor_sha256']
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_f766_inputs':4,'production_difference':'one statement-local expect, all original executable statements and fees byte-exact','prior_source_algorithms':prior['status'],'prior_component_rows':prior['whole_component_rows'],'preserved_actual_core_phase':{'exit_code':101,'bodies':0,'assets':len(files),'peak_rss_bytes':result['peak_process_group_rss_bytes'],'settled_empty':True},'new_rust_owners':0,'selection174_unchanged':True,'rust_executed':False},sort_keys=True))
