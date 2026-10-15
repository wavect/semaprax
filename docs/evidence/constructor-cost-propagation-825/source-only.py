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

# Execute every composed source-only owner, never Rust/Cargo.
import runpy
for name in ('constructor-directory-propagation-825',
             'constructor-typed-propagation-825', 'constructor-report-receiving-825'):
    runpy.run_path(str(ROOT / 'docs/evidence' / name / 'source-only.py'))
inline_path = ROOT / 'docs/evidence/constructor-inline-failure-825/exact-source-inverses.json'
inline = json.loads(inline_path.read_text())
for row in inline['owners']:
    restored = (ROOT / row['source']).read_text()
    original = (inline_path.parent / row['archive']).read_bytes()
    assert hashlib.sha256(original).hexdigest() == row['before_sha256']
    for edit in reversed(row['edits']):
        assert restored.count(edit['after']) == 1, row['source']
        restored = restored.replace(edit['after'], edit['before'], 1)
    assert restored.encode() == original, row['source']
for row in inline['children']:
    assert hashlib.sha256((ROOT / row['source']).read_bytes()).hexdigest() == row['sha256']
print('PASS prior inline5 whole inverses +5 original child hashes')
generic_path = ROOT / 'docs/evidence/borrowed-generic-type-rewrite-141387/whole-source-inverses.json'
for row in json.loads(generic_path.read_text()):
    current = (ROOT / row['source']).read_bytes()
    original = (ROOT / row['archive']).read_bytes()
    assert hashlib.sha256(current).hexdigest() == row['current_sha256']
    assert hashlib.sha256(original).hexdigest() == row['original_sha256']
    assert len(current) == row['current_bytes']
    assert len(original) == row['original_bytes']
    restored = current.decode()
    for edit in row['hooks']:
        assert restored.count(edit['after']) == 1, row['source']
        restored = restored.replace(edit['after'], edit['before'], 1)
    assert restored.encode() == original, row['source']
print('PASS generic rewrite3 whole inverses + full original/current hashes and lengths')
