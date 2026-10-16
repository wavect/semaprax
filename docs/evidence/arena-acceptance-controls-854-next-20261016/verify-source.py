#!/usr/bin/env python3
"""Finite four-input source custody and instrumentation correspondence only."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('--repo', type=Path, default=Path.cwd())
root = p.parse_args().repo.resolve()
packet = root / 'docs/evidence/arena-acceptance-controls-854-next-20261016'
data = json.loads((packet / 'source-inverses.json').read_text())
sha = lambda b: hashlib.sha256(b).hexdigest()
assert len(data['rows']) == 4
for row in data['rows']:
    path = row['source']
    before = (packet / row['before_archive']).read_bytes()
    current = (root / path).read_bytes()
    assert before == subprocess.check_output(['git', 'show', data['base'] + ':' + path], cwd=root)
    assert sha(before) == row['before_sha256'] and sha(current) == row['after_sha256']
    old, new = before.decode(), current.decode()
    at = row['offset']
    assert old[at:at + len(row['before'])] == row['before']
    assert new[at:at + len(row['after'])] == row['after']
    assert old[:at] + row['after'] + old[at + len(row['before']):] == new
    assert new[:at] + row['before'] + new[at + len(row['after']):] == old
    if path.endswith('canonical_shared.rs') or path.endswith('before_arena.rs'):
        scope = 'super::' if path.endswith('before_arena.rs') else ''
        hit = f'''Some(id) => {{
                            #[cfg(test)]
                            {scope}arena_tests::signature_lookup(true);
                            *id
                        }}'''
        miss = f'''None => {{
                            #[cfg(test)]
                            {scope}arena_tests::signature_lookup(false);'''
        assert new.count(hit) == new.count(miss) == 1
        assert new.replace(hit, 'Some(id) => *id,').replace(miss, 'None => {') == old
    if path.endswith('arena_source.rs'):
        assert 'arena854_input(' in new
        assert 'assert!(copy.contains(&old[begin..end]));' in old and 'assert!(copy.contains(&old[begin..end]));' in new
# No new owner/registration or shipping constructor/layout is introduced.
source = root / 'src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs'
text = source.read_text()
assert text.count('#[test]') == 2
assert 'const {\n        std::cell::Cell::new((0, 0))' in text
assert 'signature_lookup(true)' not in text and 'signature_lookup(false)' not in text
assert 'let (old_graph, old_sink, old_roots) = corpus(&function, shape);' in text
assert 'assert_eq!(&current.ids, old.signatures());' in text
assert 'assert_eq!(&current.states, old.states());' in text
assert 'for limit in [0, paid / 2, paid - 1, paid]' in text
assert 'assert_eq!(metrics.allocated, metrics.freed);' in text
assert 'assert_eq!(first.floor, 0);' in text
pending = json.loads((packet / 'pending-selectors.json').read_text())
assert len(pending) == 3 and len({x['selector'] for x in pending}) == 3
print(json.dumps({'status': 'PASS_SOURCE_ONLY', 'whole_input_inverses': 4, 'original_algorithms_exact_after_removing_cfg_test_hooks': True, 'existing_owners': 3, 'rust_executed': False, 'global_source_join_pending': True}, sort_keys=True))
