"""Read-only custody checks must cover authority pins and their domain hashes."""
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    'refresh_source_custody', Path(__file__).with_name('refresh-source-custody.py'))
CUSTODY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CUSTODY)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return (json.dumps(value, separators=(',', ':')) + '\n').encode()


class AuthorityCheckTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='semaprax-custody-check-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = 'src/example.rs'
        source = b'pub fn example() {}\n'
        self.put(self.source, source)
        self.put(CUSTODY.GUARD, b'unchanged overlay guard\n')
        self.put(CUSTODY.OVERLAY_PY, b'unchanged overlay decoder\n')
        self.base = {}
        domains = {}
        for name in ('hir', 'projection'):
            original = canonical({'classes': {self.source: {}}})
            original_path = f'evidence/{name}-original.json'
            self.put(original_path, original)
            self.base[name] = (original_path, digest(original))
            manifest = canonical({
                'schema': 'semaprax.source-authority-partition.v1',
                'parser_sha256': '0' * 64,
                'sources': {self.source: {'length': len(source), 'sha256': digest(source)}},
                'classes': {self.source: {'role': 'production', 'chains': [{'edges': []}]}},
                'chunks': [{'path': self.source}],
                'closure_only': [],
            })
            self.put(f'{CUSTODY.AUTHORITY_DIR}/{name}.json', manifest)
            domains[name] = {
                'current_members': [self.source],
                'production_members': [self.source],
                'fully_authenticated_outside_production': [],
                'full_manifest_sha256': digest(manifest),
                'full_raw_sha256': digest(source),
                'full_authenticated_production_sha256': digest(source),
            }
        self.put(CUSTODY.DOMAINS, (json.dumps({'domains': domains}, indent=2) + '\n').encode())

    def put(self, name, data):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    def snapshot(self):
        return {str(path.relative_to(self.root)): path.read_bytes()
                for path in self.root.rglob('*') if path.is_file()}

    def check(self):
        before = self.snapshot()
        output = io.StringIO()
        with patch.object(sys, 'argv', ['refresh-source-custody.py', '--repo', str(self.root),
                                       '--check', '--fast', '--label', 'authority-check']), \
                patch.object(CUSTODY, 'DOMAIN_BASE', self.base), \
                patch.object(CUSTODY, 'refresh_ordinary_ingress', return_value=[]), \
                patch.object(CUSTODY, 'refresh_main043_successors', return_value=[]), \
                patch.object(CUSTODY, 'refresh_overlay', return_value=None), \
                contextlib.redirect_stdout(output):
            result = CUSTODY.main()
        self.assertEqual(self.snapshot(), before, 'a check must not rewrite any file')
        return result, output.getvalue()

    def test_current_authority_and_domains_are_clean_without_writes(self):
        result, output = self.check()
        self.assertEqual(result, 0)
        self.assertFalse(output.partition('check only; overlay files that would change:')[2].strip())

    def test_source_drift_is_reported_when_the_overlay_is_unchanged(self):
        self.put(self.source, b'pub fn example() { let changed = 1; }\n')
        result, output = self.check()
        self.assertEqual(result, 1)
        for name in ('hir', 'projection'):
            self.assertIn(f'{CUSTODY.AUTHORITY_DIR}/{name}.json', output)
        self.assertIn(CUSTODY.DOMAINS, output)

    def test_domains_use_the_refreshed_manifest_preview_without_writes(self):
        for name in ('hir', 'projection'):
            path = f'{CUSTODY.AUTHORITY_DIR}/{name}.json'
            manifest = json.loads((self.root / path).read_bytes())
            manifest['sources'][self.source]['sha256'] = '0' * 64
            self.put(path, canonical(manifest))
        result, output = self.check()
        self.assertEqual(result, 1)
        for name in ('hir', 'projection'):
            self.assertIn(f'{CUSTODY.AUTHORITY_DIR}/{name}.json', output)
        self.assertNotIn(CUSTODY.DOMAINS, output,
                         'restoring a manifest pin must preserve already-correct domain hashes')


if __name__ == '__main__':
    unittest.main()
