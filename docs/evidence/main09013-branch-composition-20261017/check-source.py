"""Pure source/inverse checks. No replay, compiler or source binder execution."""
from pathlib import Path
import hashlib,json,subprocess,re
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'full-source-inverses.json').read_bytes());refused=0
for row in rows:
    raw=(R/row['path']).read_bytes();assert len(raw)==row['current_length'] and sha(raw)==row['current_sha256']
    def restore(raw,parent):
        assert sha(raw)==row['current_sha256']
        for e in parent['inverse']:
            i=e['start_byte'];a=e['after'].encode();assert raw[i:i+len(a)]==a
            raw=raw[:i]+e['before'].encode()+raw[i+len(a):]
        assert len(raw)==parent['length'] and sha(raw)==parent['sha256']
        assert raw==(R/parent['archive']).read_bytes()
        return raw
    for parent in row['parents']:
        assert restore(raw,parent)==subprocess.check_output(['git','show',parent['head']+':'+row['path']],cwd=R)
        for bad in [raw+b' ',raw[:-1],b'foreign']:
            try:restore(bad,parent)
            except AssertionError:refused+=1
            else:raise AssertionError('foreign endpoint accepted')
root=(R/'src/cleanup_plan/replay.rs').read_text();helper=(R/'src/cleanup_plan/replay/branch_clone.rs').read_text()
oldroot=(D/'before/ours/src/cleanup_plan/replay.rs.txt').read_text();mainhelper=(D/'before/main/src/cleanup_plan/replay/branch_clone.rs.txt').read_text()
start=mainhelper.index("pub(super) fn can_take(input:");end=mainhelper.index('\n#[cfg(test)]\npub(super) fn note_take',start)
assert mainhelper[start:end] in helper
begin=oldroot.index('fn plan_skeleton_paths(');start=oldroot.index('                budget.reserve_bytes(\n                    function,\n                    final_output::PLAN_SELECTION_CONTROLS,',begin);end=oldroot.index('                for edge in edges {',start)
special=oldroot[start:end]
without_counter=root.replace('                    #[cfg(test)]\n                    branch_clone::note_take();\n','',1)
assert special in without_counter
begin=root.index('fn plan_skeleton_paths(');p=root.index('                                if branch_clone::can_take(',begin)
assert root.rfind('branch_clone::TAKE_CALL_CONTROLS',begin,p)>root.rfind('budget.charge_skeleton(',begin,p)
assert root.rfind('"cleanup-plan skeleton branch clone"',begin,p)>root.index('while let Some(edge) = remaining_edges.next()',begin)
assert 'branch_clone::TAKE_BYTES,' not in root[begin:root.index('\n}\n',begin)]
oldtests=(D/'before/ours/src/cleanup_plan/replay/branch_clone/tests.rs.txt').read_text();tests=(R/'src/cleanup_plan/replay/branch_clone/tests.rs').read_text();assert tests.startswith(oldtests)
assert 'assert_eq!(current.remaining, old.remaining);' in tests
assert 'assert_eq!(current.skeleton_remaining, old.skeleton_remaining);' in tests
assert 'GENERIC_TAKES.with(Cell::get)>takes' in tests
for x in json.loads((D/'pending-selectors.json').read_bytes())['checks']:
    assert len(re.findall(r'(?m)^fn '+x['selector'].split('::')[-1]+r'\(',tests))==1
print(json.dumps({'status':'PURE_SOURCE_PASS_NO_RUST_EXECUTION','complete_parent_inverses':sum(len(row['parents']) for row in rows),'hostile_endpoints_refused':refused,'incoming_can_take_body_unchanged':True,'ours_twoedge_body_unchanged_except_counter':True,'original_assertions_retained':True,'actual_runtime_pending':True}))
