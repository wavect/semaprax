#!/usr/bin/env python3
"""Exact test-only corrections, followed by the unchanged Main65 proof on9ce inputs."""
from pathlib import Path
import argparse,hashlib,json,subprocess,sys,runpy,contextlib,io
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);R=p.parse_args().repo.resolve();D=Path(__file__).resolve().parent;BASE='9ce9ba33681c1dbcf4aad49b55a9808136fd2592';sha=lambda b:hashlib.sha256(b).hexdigest();old=lambda path:subprocess.check_output(['git','show',BASE+':'+path],cwd=R)
rows=json.loads((D/'current-to-9ce9.json').read_text());assert len(rows)==6;view={}
for r in rows:
 path=r['source'];b=(R/path).read_bytes();assert sha(b)==r['current_sha256'];s=b.decode()
 for e in r['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 b=s.encode();assert sha(b)==r['original_sha256'];assert b==old(path)==(D/'before'/path).read_bytes();view[str((R/path).resolve())]=b
expected={
 'tests/workspace/semantic_graph.rs': [('current_projection_facade_input(current_projection_root)','current_projection_facade_input(&current_projection_root)'),('main65-private-merge-20261016/','test-compile-corrections-9ce9-20261016/')],
 'src/cleanup_plan/replay/sequence_prefix.rs':[('    note_skeleton_materialization, Diagnostic, Observations, ReplayBudget, ResolvedFunction,\n','    note_skeleton_materialization, Diagnostic, Observations, ReplayBudget, ResolvedFunction,\n    SkeletonObservation,\n')],
 'src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs':[('use super::{\n','use super::{app_census, tests};\nuse super::{\n')],
 'src/cleanup_plan/replay/semantic_refusal/tests.rs':[('        Atom::UnknownCallCallee(callee) => format!("call commit has unknown callee `{callee}`"),\n','        Atom::UnknownCallCallee(callee) => format!("call commit has unknown callee `{callee}`"),\n        Atom::StatusUnknownCallee(callee) => {\n            format!("status source call has unknown callee `{callee}`")\n        }\n')]}
for path,edits in expected.items():
 s=old(path).decode()
 for a,b in edits:assert a in s;s=s.replace(a,b,1)if path.endswith('before_arena.rs')else s.replace(a,b)
 assert s==(R/path).read_text(),path
helper=(R/'src/source_reflows_merge.rs').read_text();a=helper.index('fn main65_previous(');assert helper.index('compile9ce_input(source, current)',a)<helper.index('current-to-75d8.json',a)
# Exact original packet remains immutable; replay its original algorithm with complete9ce source inputs.
O=R/'docs/evidence/main65-private-merge-20261016'
for path in subprocess.check_output(['git','ls-tree','-r','--name-only',BASE,'--',str(O.relative_to(R))],cwd=R).decode().splitlines():assert (R/path).read_bytes()==old(path)
rb0,rt0,wt0=Path.read_bytes,Path.read_text,Path.write_text;argv=sys.argv[:]
def rb(p):return view.get(str(p.resolve()),rb0(p))
def rt(p,*a,**kw):
 b=view.get(str(p.resolve()));return b.decode(kw.get('encoding')or'utf-8')if b is not None else rt0(p,*a,**kw)
def wt(p,s,*a,**kw):assert p.resolve().is_relative_to(R/'docs/evidence');return len(s)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;sys.argv=[str(O/'verify-source.py'),'--repo',str(R)];out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(O/'verify-source.py'),run_name='__main__')
 prior=json.loads(out.getvalue());assert prior['status']=='PASS_SOURCE_ONLY'
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;sys.argv=argv
print(json.dumps({'status':'PASS_SOURCE_ONLY','exact_corrected_test_sources':4,'whole_current_to_9ce_rows':6,'old_Main65_packet_byte_exact':True,'unchanged_predecessor_algorithm':prior['status'],'selectors_unchanged':True,'rust_executed':False},sort_keys=True))
