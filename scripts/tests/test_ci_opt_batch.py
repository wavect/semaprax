"""Partial-image failures collect evidence without authorizing stale binaries.

All processes below are synthetic in-memory protocol peers; no compiler or
libtest executable is started. The production main orchestration is exercised.
"""
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNNER = runpy.run_path(str(ROOT / 'scripts/ci-opt-batch.py'))
GLOBALS = RUNNER['main'].__globals__


class FocusedBatch(unittest.TestCase):
    def artifact(self, root, target, image):
        name, kind, source = ('semaprax', ['lib'], 'src/lib.rs') if image == 'lib' else (
            'owned_data', ['test'], 'tests/owned_data.rs')
        binary = target / 'debug/deps' / f'{image}-owning-image'
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_bytes(b'current immutable owning executable')
        binary.chmod(0o755)
        return binary, {
            'reason': 'compiler-artifact', 'manifest_path': str(root / 'Cargo.toml'),
            'target': {'name': name, 'kind': kind, 'src_path': str(root / source)},
            'profile': {'test': True}, 'executable': str(binary),
        }

    def build_record(self, root, target, messages, exit_code=0, timed_out=False, images=('lib',)):
        log = root / 'build.log'
        log.write_text(''.join(json.dumps(message) + '\n' for message in messages))
        command = {'label': 'build-lib', 'log': log.name,
                   'log_sha256': RUNNER['sha'](log), 'exit_code': exit_code,
                   'timed_out': timed_out}
        return RUNNER['build_images'](log, command, images, root, target)[images[0]]

    def test_only_exact_failed_production_dependency_blocks_other_images(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target = root / 'target/private'
            diagnostic = {
                'reason': 'compiler-message', 'manifest_path': str(root / 'Cargo.toml'),
                'target': {'name': 'semaprax', 'kind': ['lib'],
                           'src_path': str(root / 'src/lib.rs')},
                'message': {'level': 'error', 'message': 'unresolved production import'},
            }
            def record(message, images=('owned_data',), code=101, timeout=False):
                return self.build_record(root, target, [message, {
                    'reason': 'build-finished', 'success': False}], code, timeout, images)
            actual = record(diagnostic)
            self.assertEqual(actual['status'], 'unavailable')
            witness = actual['production_dependency_failure']
            self.assertEqual(witness['diagnostics'], [diagnostic['message']])
            self.assertEqual(witness['log_sha256'], RUNNER['sha'](root / 'build.log'))
            for images, code, timeout in [(('lib',), 101, False),
                                         (('lib', 'owned_data'), 101, False),
                                         (('owned_data',), 0, False),
                                         (('owned_data',), 101, True)]:
                self.assertNotIn('production_dependency_failure',
                                 record(diagnostic, images, code, timeout))
            for replacement in [
                {'manifest_path': str(root / 'other/Cargo.toml')},
                {'target': {'name': 'owned_data', 'kind': ['test'],
                            'src_path': str(root / 'tests/owned_data.rs')}},
                {'message': {'level': 'warning', 'message': 'a warning'}},
            ]:
                self.assertNotIn('production_dependency_failure',
                                 record({**diagnostic, **replacement}))

    def test_failed_dependency_skips_redundant_builds_without_claiming_execution(self):
        calls = []
        witness = {'build': 'build-owned_data', 'log': 'build-owned_data.log',
                   'log_sha256': 'a' * 64, 'diagnostics': [{'level': 'error'}]}
        def build(group):
            calls.append(group)
            if group == ('lib',):
                return {'lib': {'status': 'available'}}
            self.assertEqual(group, ('owned_data',))
            return {'owned_data': {'status': 'unavailable',
                                   'production_dependency_failure': witness}}
        images, attempts = RUNNER['collect_images'](
            ('lib', 'owned_data', 'cli', 'project'), False, build)
        self.assertEqual(calls, [('lib',), ('owned_data',)])
        self.assertEqual([item['attempted'] for item in attempts], [True, True, False, False])
        for name in ('cli', 'project'):
            self.assertEqual(images[name]['status'], 'unavailable')
            self.assertFalse(images[name]['attempted'])
            self.assertIsNone(images[name]['build'])
            self.assertEqual(images[name]['production_dependency_failure'], witness)
            row = RUNNER['unavailable_filter']({'binary': name}, 0, images[name])
            self.assertFalse(row['executed'])
            self.assertIsNone(row['observed_test_count'])

    def test_failed_build_cannot_adopt_even_an_emitted_existing_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target = root / 'target/private'
            _, message = self.artifact(root, target, 'lib')
            for code, finished, timed_out in [(101, False, False), (101, True, False), (0, True, True)]:
                record = self.build_record(root, target, [message, {
                    'reason': 'build-finished', 'success': finished}], code, timed_out)
                self.assertEqual(record['status'], 'unavailable')
                self.assertNotIn('binary', record)
                self.assertEqual(record['build_exit_code'], code)
                self.assertTrue(record['build_log_sha256'])

    def test_success_needs_one_exact_selected_current_manifest_and_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target = root / 'target/private'
            _, message = self.artifact(root, target, 'lib')
            finished = {'reason': 'build-finished', 'success': True}
            self.assertEqual(self.build_record(root, target, [message, finished])['status'], 'available')
            for messages in ([finished], [message, message, finished], [message],
                             [message, finished, finished]):
                self.assertEqual(self.build_record(root, target, messages)['status'], 'unavailable')
            for field, value in [('manifest_path', str(root / 'foreign/Cargo.toml')),
                                 ('target', {**message['target'], 'src_path': str(root / 'foreign.rs')})]:
                wrong = {**message, field: value}
                self.assertEqual(self.build_record(root, target, [wrong, finished])['status'], 'unavailable')

    def test_binary_drift_symlink_and_foreign_target_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target = root / 'target/private'
            binary, _ = self.artifact(root, target, 'lib')
            snapshot = RUNNER['binary_snapshot'](binary, root, target)
            binary.write_bytes(b'tampered executable')
            with self.assertRaises(RuntimeError):
                RUNNER['verify_binary'](snapshot, root, target)
            binary.unlink(); binary.symlink_to(root / 'outside')
            with self.assertRaises(RuntimeError):
                RUNNER['binary_snapshot'](binary, root, target)
            outside = root / 'old-executable'; outside.write_bytes(b'old'); outside.chmod(0o755)
            with self.assertRaises(RuntimeError):
                RUNNER['binary_snapshot'](outside, root, target)

    def test_historical_happy_command_group_is_preserved_and_failure_stays_failure(self):
        calls = []
        def successful(group):
            calls.append(group)
            return {image: {'status': 'available'} for image in group}
        images, attempts = RUNNER['collect_images'](('lib', 'hir_module_boundaries'), True, successful)
        self.assertEqual(calls, [('lib', 'hir_module_boundaries')])
        self.assertEqual(len(attempts), 1)
        calls.clear()
        def failing(group):
            calls.append(group)
            return {image: {'status': 'unavailable' if len(group) > 1 else 'available'} for image in group}
        images, attempts = RUNNER['collect_images'](('lib', 'hir_module_boundaries'), True, failing)
        self.assertEqual(calls, [('lib', 'hir_module_boundaries'), ('lib',), ('hir_module_boundaries',)])
        receipt = {'images': images, 'build_attempts': attempts,
                   'tests': [{'status': 'passed', 'completed': True, 'passed': 1}],
                   'clippy': {'passed': True}}
        self.assertFalse(RUNNER['qualification_passed'](receipt, 1, 1))

    def orchestration(self, *, failed_image=None, tamper_after_list=False, clippy_code=0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); target = root / 'target/private'; out = root / 'diagnostics'
            (root / 'scripts').mkdir()
            rows = [{'owner': image, 'binary': image, 'filter': f'{image}_case',
                     'exact': True, 'expected_tests': 1} for image in ('lib', 'owned_data')]
            (root / 'scripts/selection.json').write_text(json.dumps({
                'schema': 'semaprax.hosted-opt-batch-selection.v1',
                'filters': 2, 'expected_tests': 2, 'commands': rows}))
            calls = []
            test = self
            class Process:
                def __init__(self, args, *, cwd, env, stdout, **kwargs):
                    calls.append(args)
                    self.pid = 123456789
                    self.returncode = 0
                    output = ''
                    if args[:2] == ['cargo', 'test']:
                        image = 'lib' if '--lib' in args else 'owned_data'
                        _, artifact = test.artifact(root, target, image)
                        self.returncode = 101 if image == failed_image else 0
                        # Even failure deliberately emits a plausible old image;
                        # build-finished/exit still prohibit adopting it.
                        output = json.dumps(artifact) + '\n' + json.dumps({
                            'reason': 'build-finished', 'success': self.returncode == 0}) + '\n'
                    elif args[:2] == ['cargo', 'clippy']:
                        self.returncode = clippy_code
                    elif args[0] != 'cargo':
                        if '--list' in args:
                            output = args[1] + ': test\n'
                            if tamper_after_list and args[1] == 'lib_case':
                                Path(args[0]).write_bytes(b'tampered after discovery')
                        else:
                            output = 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'
                    stdout.write(output.encode())
                def wait(self, timeout=None):
                    return self.returncode
            def metadata(args):
                return 'rustc 1.97.1 synthetic' if args[0] == 'rustc' else 'cargo synthetic'
            replacements = {'ROOT': root, 'OUT': out, 'TARGET': target,
                            'SELECTIONS': {'next': ('selection.json', 2, 2, ('lib', 'owned_data'))},
                            'state': lambda expected: {'head': expected, 'tree': 'fixed-tree'},
                            'text': metadata,
                            'memory_sample': lambda group: {'mem_available_bytes': 100,
                                                           'command_group_rss_bytes': 0}}
            with patch.dict(GLOBALS, replacements), patch.dict(os.environ, {
                    'GITHUB_SHA': 'a' * 40, 'CLANG': 'clang', 'SEMAPRAX_ARCHIVER': 'ar'}, clear=True), \
                    patch('sys.argv', ['ci-opt-batch.py', '--selection', 'next']), \
                    patch.object(RUNNER['subprocess'], 'Popen', Process):
                code = RUNNER['main']()
            return code, json.loads((out / 'receipt.json').read_text()), calls

    def test_partial_compile_failure_still_runs_other_owner_and_production_clippy(self):
        code, receipt, calls = self.orchestration(failed_image='lib')
        self.assertEqual(code, 1)
        self.assertEqual(receipt['status'], 'failed')
        self.assertTrue(receipt['clippy']['passed'])
        self.assertEqual(receipt['unavailable_images'], ['lib'])
        missing = receipt['tests'][0]
        self.assertEqual(missing['status'], 'not-executed')
        self.assertFalse(missing['attempted'] or missing['executed'] or missing['completed'])
        self.assertIsNone(missing['discovered_count'])
        self.assertIsNone(missing['observed_test_count'])
        self.assertNotIn('passed', missing)
        self.assertEqual(receipt['tests'][1]['status'], 'passed')
        self.assertFalse(receipt['summary_counts_complete'])
        self.assertFalse(any('lib_case' in command for command in calls))
        self.assertEqual([command[1] for command in calls if command[0] == 'cargo'],
                         ['fetch', 'test', 'test', 'clippy'])
        for command in calls:
            if command[:2] in (['cargo', 'test'], ['cargo', 'clippy']):
                self.assertEqual(command[command.index('--jobs') + 1], '1')

    def test_discovery_time_tamper_quarantines_owner_but_collects_other_outcomes(self):
        code, receipt, calls = self.orchestration(tamper_after_list=True)
        self.assertEqual(code, 1)
        self.assertEqual(receipt['tests'][0]['status'], 'binary-identity-failed')
        self.assertFalse(receipt['tests'][0]['executed'])
        self.assertEqual(receipt['tests'][1]['status'], 'passed')
        self.assertTrue(receipt['clippy']['passed'])
        self.assertFalse(any('lib_case' in command and '--nocapture' in command for command in calls))

    def test_overall_success_requires_every_owner_count_and_clippy(self):
        code, receipt, _ = self.orchestration()
        self.assertEqual(code, 0)
        self.assertEqual(receipt['status'], 'passed')
        self.assertEqual(receipt['passed_tests'], 2)
        self.assertTrue(receipt['summary_counts_complete'])
        code, receipt, _ = self.orchestration(clippy_code=101)
        self.assertEqual(code, 1)
        self.assertEqual(receipt['status'], 'failed')
        self.assertFalse(receipt['clippy']['passed'])
