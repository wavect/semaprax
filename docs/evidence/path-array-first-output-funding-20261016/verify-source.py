#!/usr/bin/env python3
import hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
P=Path(__file__).resolve().parent
j=json.loads((P/'source-inverses.json').read_text())
def digest(s): return hashlib.sha256(s.encode()).hexdigest()
def body(s,name):
    marker='fn '+name+'('
    start=s.index(marker)
    end=s.index('\n}\n',start)+3
    return s[start:end]
for row in j['rows']:
    actual=(ROOT/row['path']).read_text()
    assert digest(actual)==row['after_sha256'],row['path']
    assert actual==row['after']
    before=(P/row['before_archive']).read_text()
    assert before==row['before']==subprocess.check_output(['git','show',j['base']+':'+row['path']],cwd=ROOT).decode()
    assert digest(before)==row['before_sha256']
    assert actual.replace(row['inverse'][0]['old'],row['inverse'][0]['new'])==before
rows={r['path']:r for r in j['rows']}
replay=rows['src/cleanup_plan/replay.rs']
main=(ROOT/'src/cleanup_plan/replay/path_array/main696.rs').read_text()
restored=replay['after']
for name in ['sequence_skeleton_paths','append_expr_paths','split_status_paths','split_contract']:
    old=body(replay['before'],name)
    assert body(main,name)==old,name
    restored=restored.replace(body(restored,name),old)
assert restored==replay['before'],'unrelated replay changes'
old=rows['src/cleanup_plan/replay/path_array.rs']['before']
for name in ['receiving_bytes','minimum']:
    assert body(main,name)==body(old,name),name
before=rows['src/cleanup_plan/replay/path_array/before.rs']
assert before['after'].replace('use super::super::*;\n','',1)==before['before']
t=rows['src/cleanup_plan/replay/path_array/tests.rs']
original=t['after'].replace('use super::super::*;\n','',1).replace('use super::main696::{append_expr_paths, sequence_skeleton_paths, split_status_paths, split_contract, hir_skeleton_paths, receiving_bytes};\n','',1)
original=original.replace('let root = crate::source_reflows_merge::main696_input(\n        "src/cleanup_plan/replay.rs", include_str!("../../replay.rs"));','let root = include_str!("../../replay.rs");',1)
fixture_block="""                    // Exact seed + receiver work: every third seed is live.
                    let live = count / 3;
                    let units = match receiver {
                        0 => 2 * count,
                        1 => 2 * count + 1 + 3 * live,
                        2 => 2 * count + 1 + 6 * live,
                        _ => 2 * count + 6 * live,
                    };
                    old_budget.reserve_skeleton(function, units).unwrap();
                    budget.reserve_skeleton(function, units).unwrap();
"""
assert original.count(fixture_block)==1
original=original.replace(fixture_block,'',1)
fixture_block="""                        // count seed pushes, count append pushes, three later pushes.
                        budget.reserve_skeleton(function, 2 * count + 3)?;
"""
assert original.count(fixture_block)==1
original=original.replace(fixture_block,'',1)
assert original==t['before'],'old test body changed'
for r in json.loads((P/'new-owners.json').read_text()):
    assert digest((ROOT/r['path']).read_text())==r['sha256']
assert 'use super::*;' not in (ROOT/'src/cleanup_plan/replay/path_array.rs').read_text()
assert len((ROOT/'src/cleanup_plan/replay.rs').read_text().splitlines())<=7993
assert all(len((ROOT/r['path']).read_text().splitlines())<=1500 for r in json.loads((P/'new-owners.json').read_text()))
print('PASS: 4 whole inverses; complete Main helper/four receivers; all original tests; 2 new owners; line caps; no unrelated replay changes')
