"""Read-only whole-source projection; no compiler/runtime result is implied."""
from pathlib import Path
import hashlib
import json
import argparse

ap = argparse.ArgumentParser()
ap.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[3])
a = ap.parse_args()
r = a.repo.resolve()
d = r / 'docs/evidence/status-case-proof-816'
rows = json.loads((d / 'exact-source-inverses.json').read_bytes())
proof = []
for row in rows:
    raw = (r / row['source']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == row['current_sha256'], row['source']
    text = raw.decode('utf-8')
    for edit in row['edits']:
        assert text.count(edit['new']) == 1, row['source']
        text = text.replace(edit['new'], edit['before'], 1)
    old = (r / row['original']).read_bytes()
    assert text.encode('utf-8') == old, row['source']
    assert hashlib.sha256(old).hexdigest() == row['original_sha256'], row['source']
    proof.append({'source': row['source'], 'whole_projection': True,
                  'current_sha256': row['current_sha256'], 'original_sha256': row['original_sha256']})
print(json.dumps({'status': 'SOURCE_ONLY_NO_COMPILER_OR_RUNTIME', 'rows': len(proof), 'verified': proof}, indent=2))
