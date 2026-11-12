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
                patch.object(CUSTODY, 'refresh_v20_ingress', return_value=[]), \
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


class V20IngressRefreshTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='semaprax-v20-ingress-')
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.source = 'src/controller.rs'
        self.previous = 'prévious = [1, 2]\n'.encode()
        self.current = 'current = [1, 2]\n'.encode()
        self.live = 'live = [1, 3]\n// reste é\n'.encode()
        source_set = patch.object(CUSTODY, 'V20_SOURCES', (self.source,))
        source_set.start()
        self.addCleanup(source_set.stop)
        self.row = {
            'source': self.source,
            'current': {'length': len(self.current), 'sha256': digest(self.current)},
            'previous': {'length': len(self.previous), 'sha256': digest(self.previous)},
            'edits': [[0, len(self.current), self.previous.decode()]],
        }
        self.packet = {
            'v20_shared_ingress': {
                'schema': 'semaprax.v20-shared-source-ingress.v2',
                'source_kind': 'ACTUAL_CURRENT_TO_ENTIRE_PRE_V20_PHYSICAL_STAGE',
                'sources': [self.source],
                'records': [self.row],
            },
            'retained': {'sentinel': True},
        }
        self.repo = CUSTODY.Repo(self.root, False)
        self.repo.write(self.source, self.live)
        self.repo.write('e/' + digest(self.current), self.current)
        self.repo.write('e/' + digest(self.previous), self.previous)
        self.raw = (json.dumps(self.packet, separators=(',', ':'), ensure_ascii=False) + '\n').encode()
        self.repo.write(CUSTODY.V20_INGRESS, self.raw)
        self.repo.written.clear()

    def test_refresh_reconstructs_the_sealed_source_and_check_only_previews(self):
        check = CUSTODY.Repo(self.root, True)
        archive = 'e/' + digest(self.live)
        self.assertEqual(CUSTODY.refresh_v20_ingress(check), [self.source])
        self.assertEqual(check.written, [archive, CUSTODY.V20_INGRESS])
        self.assertFalse((self.root / archive).exists())
        self.assertEqual(self.repo.raw(CUSTODY.V20_INGRESS), self.raw)

        self.assertEqual(CUSTODY.refresh_v20_ingress(self.repo), [self.source])
        packet = json.loads(self.repo.raw(CUSTODY.V20_INGRESS))
        self.assertEqual(packet['retained'], self.packet['retained'])
        row = packet['v20_shared_ingress']['records'][0]
        self.assertEqual(row['previous'], self.row['previous'])
        self.assertEqual(self.repo.raw(archive), self.live)
        restored = self.live
        for start, end, prior in reversed(row['edits']):
            restored = restored[:start] + prior.encode() + restored[end:]
        self.assertEqual(restored, self.previous)
        self.assertEqual(CUSTODY.refresh_v20_ingress(self.repo), [])

    def test_corrupt_retained_archive_leaves_the_packet_unchanged(self):
        self.repo.write('e/' + digest(self.previous), b'corrupt archive\n')
        self.repo.written.clear()
        with self.assertRaisesRegex(AssertionError, 'invalid V20 source archive'):
            CUSTODY.refresh_v20_ingress(self.repo)
        self.assertEqual(self.repo.written, [])
        self.assertEqual(self.repo.raw(CUSTODY.V20_INGRESS), self.raw)


class DistinctOwnerOffsetsTests(unittest.TestCase):
    def test_refresh_relocates_both_occurrences_and_refuses_new_ones(self):
        carrier = 'tests/carrier.rs'
        source = 'src/example.rs'
        literal = b'include_str!("../src/example.rs")'
        previous = {(carrier, source): (0, len(literal) + 1)}
        guard = f' finite_distinct_owners={previous!r}\n'
        carriers = {carrier: {'base': '../'}}
        moved = b'// heading\n' + literal + b'\n' + literal
        current = {(carrier, source): (moved.index(literal), moved.rindex(literal))}
        self.assertEqual(
            CUSTODY.refresh_distinct_owner_offsets(guard, carriers, {carrier: moved}),
            f' finite_distinct_owners={current!r}\n',
        )
        with self.assertRaises(AssertionError):
            CUSTODY.refresh_distinct_owner_offsets(
                guard, carriers, {carrier: moved + b'\n' + literal})


class DeclaredHistoryRefreshTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='semaprax-declared-history-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.current = b'first\nold current\nlast\n'
        self.previous = b'first\nsealed predecessor\nlast\n'
        self.source = 'src/owner.rs'
        self.part = CUSTODY.DECLARED_HISTORY + '/part-0.json'
        self.put(self.source, self.current)
        self.put('e/' + digest(self.current), self.current)
        self.put('e/' + digest(self.previous), self.previous)
        self.row = dict(source=self.source, provenance='independent-original',
                        current_bytes=len(self.current), current_sha256=digest(self.current),
                        current_archive='e/' + digest(self.current),
                        before_bytes=len(self.previous), before_sha256=digest(self.previous),
                        before_archive='e/' + digest(self.previous),
                        inverse=[[6, 'old current\n', 'sealed predecessor\n']],
                        unknown_metadata={'retain': True})
        raw = canonical([self.row])
        self.put(self.part, raw)
        holder = (f'const PARTS: &[(&str, &str, usize, &str)] = &[("{self.part}",\n'
                  f'include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/{self.part}")),\n'
                  f'{len(raw)}, "{digest(raw)}")];\n')
        self.put(CUSTODY.DECLARED_HISTORY_RS, holder.encode())

    def put(self, path, value):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(value)

    def snapshot(self):
        return {str(p.relative_to(self.root)): p.read_bytes()
                for p in self.root.rglob('*') if p.is_file()}

    def test_refresh_preserves_the_complete_sealed_predecessor_and_metadata(self):
        live = 'new é header\nfirst\nnew current\nlast\nnew tail\n'.encode()
        self.put(self.source, live)
        repo = CUSTODY.Repo(self.root, False)
        self.assertEqual(CUSTODY.refresh_declared_history(repo), [self.source])
        row = json.loads((self.root / self.part).read_bytes())[0]
        for key in ('before_bytes', 'before_sha256', 'before_archive', 'provenance', 'unknown_metadata'):
            self.assertEqual(row[key], self.row[key])
        self.assertEqual(row['current_sha256'], digest(live))
        self.assertEqual((self.root / row['current_archive']).read_bytes(), live)
        restored = live
        for start, after, before in reversed(row['inverse']):
            after, before = after.encode(), before.encode()
            self.assertEqual(restored[start:start + len(after)], after)
            restored = restored[:start] + before + restored[start + len(after):]
        self.assertEqual(restored, self.previous)
        self.assertEqual((self.root / self.row['current_archive']).read_bytes(), self.current)
        raw = (self.root / self.part).read_bytes()
        holder = (self.root / CUSTODY.DECLARED_HISTORY_RS).read_text()
        self.assertIn(digest(raw), holder)
        self.assertIn(str(len(raw)), holder)
        self.assertEqual(CUSTODY.refresh_declared_history(repo), [])

    def test_check_reports_drift_without_writing(self):
        self.put(self.source, b'changed\n')
        before = self.snapshot()
        repo = CUSTODY.Repo(self.root, True)
        self.assertEqual(CUSTODY.refresh_declared_history(repo), [self.source])
        self.assertIn(self.part, repo.written)
        self.assertIn(CUSTODY.DECLARED_HISTORY_RS, repo.written)
        self.assertEqual(self.snapshot(), before)

    def test_corrupt_predecessor_archive_is_refused_before_writes(self):
        self.put(self.source, b'changed\n')
        self.put(self.row['before_archive'], b'forged predecessor\n')
        before = self.snapshot()
        with self.assertRaises(AssertionError):
            CUSTODY.refresh_declared_history(CUSTODY.Repo(self.root, False))
        self.assertEqual(self.snapshot(), before)


if __name__ == '__main__':
    unittest.main()
