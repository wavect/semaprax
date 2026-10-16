#!/usr/bin/env python3
"""Read-only exact source/preimage verification, not executable qualification."""
from pathlib import Path
import hashlib
import json
import subprocess
ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
BASE = 'a82e00cc83f99b16249181c45710d962598577db'
sha = lambda s: hashlib.sha256(s.encode()).hexdigest()
rows = json.loads((HERE / 'current-to-a82e.json').read_text())
assert [r['source'] for r in rows] == ['src/hir/expr_nodes.rs', 'src/bounded_output.rs']
for row in rows:
    source = row['source']
    before = (HERE / 'before' / source.replace('/', '__')).read_text()
    assert before == subprocess.check_output(['git', 'show', BASE + ':' + source], cwd=ROOT).decode()
    current = (ROOT / source).read_text()
    assert sha(current) == row['current_sha256']
    for wrong in [current + ' ', '', before]:
        assert sha(wrong) != row['current_sha256']
    for edit in row['edits']:
        assert edit['new'] and current.count(edit['new']) == 1
        current = current.replace(edit['new'], edit['old'], 1)
    assert current == before and sha(current) == row['original_sha256']
old = (HERE / 'before/src__hir__expr_nodes.rs').read_text()
new = (ROOT / 'src/hir/expr_nodes.rs').read_text()
marker = '#[cfg(test)]\nmod residual_payload_tests {'
assert old.split(marker)[0] == new.split(marker)[0]  # complete production surface
start = '    #[test]\n    fn lexical_residual_receiving_keeps_old_producer_order_floor_and_restored_sticky_owner() {'
assert old.split(start)[0] == new.split(start)[0]  # all preceding original controls
assert old.split('    fn leaf() -> ResolvedExpr {')[1] == new.split('    fn leaf() -> ResolvedExpr {')[1]
for assertion in ['assert!(!inner_refused);', 'assert_eq!(inner_used, debit);', 'assert!(refused);', 'assert_eq!(used, 0);', 'assert_eq!(first.unwrap().requested, 17);']:
    assert assertion in new
old = (HERE / 'before/src__bounded_output.rs').read_text()
new = (ROOT / 'src/bounded_output.rs').read_text()
old_cfg = '''#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]'''
new_cfg = '''#[cfg(any(
    test,
    all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )
))]'''
anchor = '\npub(crate) use source_owner_scope::source_owner_scope;'
assert new.replace(new_cfg + anchor, old_cfg + anchor, 1) == old
for unix in [False, True]:
    for admitted_platform in [False, True]:
        for test in [False, True]:
            prior = unix and admitted_platform
            current = test or prior
            if not test:
                assert prior == current
scope = 'src/bounded_output/source_owner_scope.rs'
assert (ROOT / scope).read_bytes() == subprocess.check_output(['git', 'show', BASE + ':' + scope], cwd=ROOT)
assert len((ROOT / 'src/hir/expr_nodes.rs').read_text().splitlines()) < 1500
assert len(new.splitlines()) < 1500
print('PASS: 2 complete preimages, 6 wrong SHA preimages, unchanged production macro/debit/API predicate; source-only; final adapter hook pending')
