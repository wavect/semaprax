"""Verify full current bytes and immutable original inverse; no Rust execution."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
rows = json.loads((HERE / 'source-inverses.json').read_text())
for row in rows:
    current = (ROOT / row['source']).read_bytes()
    original = (ROOT / row['archive']).read_bytes()
    assert hashlib.sha256(current).hexdigest() == row['current_sha256'], row['source']
    assert hashlib.sha256(original).hexdigest() == row['original_sha256'], row['archive']
    restored = current.decode()
    for edit in row['edits']:
        assert restored.count(edit['new']) == 1, row['source']
        restored = restored.replace(edit['new'], edit['before'], 1)
    assert restored.encode() == original, row['source']
print(f'PASS {len(rows)} complete source inverses; all original archives exact')
packet = json.loads((HERE / 'pending-selectors.json').read_text())
for row in packet['new'] + packet['required_existing']:
    current = (ROOT / row['source']).read_bytes()
    assert hashlib.sha256(current).hexdigest() == row['source_sha256'], row['source']
    name = row['selector'].rsplit('::', 1)[1]
    assert ('fn ' + name + '(').encode() in current, row['selector']
print(f"PASS authored source names/bindings: {len(packet['new'])} new, {len(packet['required_existing'])} required existing; not compiler registration/runtime")

import runpy
runpy.run_path(str(ROOT / 'docs/evidence/constructor-signature-rewrite-825/source-only.py'))

# All incoming prior ordered hooks and archives remain exact. The only active
# change per source is an additional authenticated current-to-b758 full prefix.
for prior in (HERE / 'prior-manifests').glob('*.json'):
    name = prior.stem
    filenames = {'borrowed-generic-type-rewrite-141387': 'whole-source-inverses.json',
                 'constructor-inline-failure-825': 'exact-source-inverses.json'}
    old = json.loads(prior.read_text())
    active = json.loads((ROOT / 'docs/evidence' / name / filenames.get(name, 'source-inverses.json')).read_text())
    old_rows = old if isinstance(old, list) else old['owners']
    active_rows = active if isinstance(active, list) else active['owners']
    assert len(old_rows) == len(active_rows)
    for before, now in zip(old_rows, active_rows):
        key = 'hooks' if 'hooks' in before else 'edits'
        old_hooks, hooks = before[key], now[key]
        if hooks != old_hooks:
            assert len(hooks) == len(old_hooks) + 1
            assert (hooks[:-1] if name == 'constructor-inline-failure-825' else hooks[1:]) == old_hooks
        for key, value in before.items():
            if key not in ('hooks', 'edits', 'current_sha256', 'current_bytes'):
                assert now[key] == value
print('PASS all seven prior metadata histories and old archive fields preserved')
