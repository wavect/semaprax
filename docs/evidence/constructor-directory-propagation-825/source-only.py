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
