#!/usr/bin/env python3
"""Bounded source-only reconciliation; immutable component verifier is unchanged."""
import argparse, hashlib, json, runpy, subprocess, sys
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument('--repo', type=Path, default=Path.cwd())
root = parser.parse_args().repo.resolve()
packet = root / 'docs/evidence/path-boundary-755-reconciliation-2c430-20261016'
data = json.loads((packet / 'binding.json').read_text())
sha = lambda b: hashlib.sha256(b).hexdigest()
def git(ref, path):
    return subprocess.check_output(['git', 'show', ref + ':' + path], cwd=root)
projected = {}
for row in data['projection_rows']:
    path = row['source']
    current = (root / path).read_bytes()
    component = (packet / row['component_archive']).read_bytes()
    assert current == (packet / row['current_archive']).read_bytes()
    assert sha(current) == row['current_sha256']
    assert sha(component) == row['component_sha256']
    assert component == git(data['component'], path)
    projected[root / path] = component
changed = subprocess.check_output(['git', 'diff', '--name-only', data['base'], 'HEAD', '--', 'src'], cwd=root, text=True).splitlines()
assert set(changed) == {row['source'] for row in data['changed_rust']}
for row in data['changed_rust']:
    path = row['source']
    current = (root / path).read_bytes()
    assert sha(current) == row['sha256']
    assert len(current.decode().splitlines()) == row['lines']
    component = git(data['component'], path)
    if path == 'src/cleanup_plan/replay/lexical_boundary/paid.rs':
        old = b'let site=$site;let domain=$domain;'
        new = b'let site:&crate::hir::ExpressionId=$site;let domain=$domain;'
        assert component.count(old) == 1
        component = component.replace(old, new)
    assert current == component, path
child = 'src/cleanup_plan/replay/selected_context/child.rs'
assert (root / child).read_bytes() == git(data['base'], child)
old_packet = 'docs/evidence/path-boundary-borrowed-chain-paid-781-next-20261016'
assert not subprocess.check_output(['git', 'diff', data['component'], '--', old_packet], cwd=root)
# Authenticate finite raw whole inputs first. Replay the original verifier
# against its exact component domain without replacing any physical source.
read_bytes, read_text, argv = Path.read_bytes, Path.read_text, sys.argv
try:
    def virtual_bytes(path):
        return projected.get(path.resolve(), None) if path.resolve() in projected else read_bytes(path)
    def virtual_text(path, *args, **kwargs):
        if path.resolve() in projected:
            encoding = kwargs.get('encoding') or (args[0] if args else None) or 'utf-8'
            return projected[path.resolve()].decode(encoding)
        return read_text(path, *args, **kwargs)
    Path.read_bytes, Path.read_text = virtual_bytes, virtual_text
    sys.argv = ['verify-source.py', '--repo', str(root)]
    runpy.run_path(str(root / old_packet / 'verify-source.py'), run_name='__main__')
finally:
    Path.read_bytes, Path.read_text, sys.argv = read_bytes, read_text, argv
print(json.dumps({'status': 'PASS_SOURCE_ONLY', 'whole_projection_rows': len(projected), 'changed_rust': len(changed), 'compile_fix_retained': True, 'rust_executed': False, 'global_join_pending': True}))
