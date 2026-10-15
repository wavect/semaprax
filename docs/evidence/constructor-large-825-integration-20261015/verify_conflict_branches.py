"""Validate both retained metadata branches against immutable incoming commits."""
from pathlib import Path
import hashlib
import json
import subprocess

root = Path(__file__).resolve().parents[3]
here = Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()
def blob(revision, path):
    return subprocess.check_output(['git', 'show', revision + ':' + path], cwd=root)
results = []
for manifest in sorted(here.glob('conflicts-*.json')):
    for conflict in json.loads(manifest.read_text()):
        revision, name = conflict['incoming_revision'], conflict['path']
        for side in ['base', 'root', 'incoming']:
            raw = (here / 'conflict-inputs' / revision / side / name).read_bytes()
            assert sha(raw) == conflict[side + '_sha256']
        incoming = json.loads((here / 'conflict-inputs' / revision / 'incoming' / name).read_text())
        assert incoming == json.loads(blob(revision, name))
        rows = incoming if isinstance(incoming, list) else incoming['owners']
        for row in rows:
            text = blob(revision, row['source']).decode()
            if 'current_sha256' in row:
                assert sha(text.encode()) == row['current_sha256']
            key = 'hooks' if 'hooks' in row else 'edits'
            edits = row[key]
            new = 'after' if key == 'hooks' or (edits and 'after' in edits[0]) else 'new'
            for edit in reversed(edits) if 'constructor-inline-failure' in name else edits:
                assert text.count(edit[new]) == edit.get('count', 1), (revision, name, row['source'])
                text = text.replace(edit[new], edit['before'], edit.get('count', 1))
            archive = Path(row['archive'])
            if not (root / archive).exists():
                archive = Path(name).parent / archive
            assert text.encode() == (root / archive).read_bytes()
            results.append({'revision': revision, 'manifest': name, 'source': row['source'],
                            'old_hooks_preserved': len(edits), 'original_sha256': sha(text.encode())})

rebindings = json.loads((here / 'selector-rebindings.json').read_text())
candidates = ['b244a1fca', '457049089', 'b758c3cdc', '5ca19ed2d', '95e6435fe',
              '940fcf220', 'bb6e11a1e']
for row in rebindings:
    source = (root / row['source']).read_bytes()
    assert sha(source) == row['current_sha256']
    assert ('fn ' + row['selector'].rsplit('::', 1)[1] + '(').encode() in source
    matches = []
    for revision in candidates:
        try:
            if sha(blob(revision, row['source'])) == row['prior_sha256']:
                matches.append(revision)
        except subprocess.CalledProcessError:
            pass
    assert matches, row
proof = {'status': 'SOURCE_ONLY_NO_RUST_EXECUTION', 'incoming_original_rows': len(results),
         'retained_conflict_manifests': 8, 'all_incoming_hooks_and_archives_preserved': True,
         'rows': results, 'authenticated_selector_rebindings': len(rebindings)}
(here / 'conflict-branch-source-proof.json').write_text(json.dumps(proof, indent=2) + '\n')
print(f'PASS both conflict branches retained; {len(results)} incoming whole-source inverses; {len(rebindings)} authenticated selector bindings')
