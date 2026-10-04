import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location('proof_cache_worker', ROOT / 'law16_proof_verdict_cache_worker.py')
WORKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WORKER)
CAPTURE_SPEC = importlib.util.spec_from_file_location('proof_cache_capture', ROOT / 'law16_guest_cache_proof.py')
CAPTURE = importlib.util.module_from_spec(CAPTURE_SPEC)
CAPTURE_SPEC.loader.exec_module(CAPTURE)


class ProofVerdictCacheWorkerTests(unittest.TestCase):
    def test_native_arm64_pin_bytes_need_separate_expected_sha(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = {'schema': CAPTURE.PIN_SCHEMA, 'profile': 'native_arm64_semaprax_z3_project_proof_check',
                     'semaprax_commit': CAPTURE.SEMAPRAX_COMMIT,
                     'tools': {'semaprax': 'sha256:' + 'a' * 64, 'z3': 'sha256:' + 'b' * 64},
                     'z3_libraries': {},
                     'image': 'example.invalid/arm64@sha256:' + 'd' * 64,
                     'image_sha256': 'sha256:' + 'e' * 64}
            path = root / 'pins.json'
            path.write_bytes((json.dumps(value, separators=(',', ':'), sort_keys=True) + '\n').encode())
            self.assertEqual(CAPTURE.read_arm64_pins(path, CAPTURE.digest(path)), value)
            with self.assertRaisesRegex(ValueError, 'independently approved'):
                CAPTURE.read_arm64_pins(path, 'sha256:' + '0' * 64)

    def test_static_z3_pin_map_is_valid_and_worker_accepts_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root)
            result['z3_library_pins'] = {}
            result['file_inventory']['semaprax_z3_project_proof_check'].pop('/tools/libz3.so')
            for sample in result['samples']:
                for field in ('before_reset', 'before_cold', 'before_warm'):
                    sample[field].pop('/tools/libz3.so')
            self.assertTrue(WORKER.validate_result(result, root))

    def test_wrapper_cli_has_one_expected_pins_option(self):
        with self.assertRaises(SystemExit) as exit_info:
            CAPTURE.main(['--help'])
        self.assertEqual(exit_info.exception.code, 0)

    def test_image_must_match_architecture_and_document_python3(self):
        image = [{'variants': [{'platform': {'architecture': 'arm64'},
                                'config': {'history': [{'created_by': 'RUN install python3'}]}}]}]
        CAPTURE.verify_python_image(image, 'arm64')
        with self.assertRaisesRegex(ValueError, 'does not document'):
            CAPTURE.verify_python_image([{'variants': [{'platform': {'architecture': 'arm64'},
                                                        'config': {'history': []}}]}], 'arm64')

    def test_zero_byte_inventory_file_has_an_empty_residency_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'empty'
            path.touch()
            self.assertEqual(WORKER.resident(path), {'pages': 0, 'resident_pages': 0})

    def test_native_arm64_review_requires_independent_pin_sha(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pins = {'schema': CAPTURE.PIN_SCHEMA, 'profile': 'native_arm64_semaprax_z3_project_proof_check',
                    'semaprax_commit': CAPTURE.SEMAPRAX_COMMIT,
                    'tools': {'semaprax': 'sha256:' + 'a' * 64, 'z3': 'sha256:' + 'b' * 64},
                    'z3_libraries': {'libz3.so': 'sha256:' + 'c' * 64},
                    'image': 'example.invalid/arm64@sha256:' + 'd' * 64,
                    'image_sha256': 'sha256:' + 'e' * 64}
            pin_path = root / 'source-pins.json'
            pin_path.write_bytes((json.dumps(pins, separators=(',', ':'), sort_keys=True) + '\n').encode())
            (root / 'receipt.json').write_text(json.dumps({
                'schema': 'semaprax.bend2-law-benchmark.proof-verdict-cache-capture.v2',
                'status': 'captured', 'container_exit_code': 0, 'container_removed': True,
                'image': pins['image'], 'image_digest': pins['image_sha256'], 'pins_sha256': CAPTURE.digest(pin_path),
                'source_pins': pins, 'nonclaims': CAPTURE.NONCLAIMS,
            }))
            with self.assertRaisesRegex(ValueError, 'independently approved expected SHA-256'):
                CAPTURE.review(root)

    def result(self, root, profile='native_arm64_semaprax_z3_project_proof_check'):
        samples = []
        route = WORKER.PROFILES[profile]['route']
        if profile.startswith('native_arm64'):
            tool_pins = {'semaprax': 'sha256:' + 'a' * 64, 'z3': 'sha256:' + 'b' * 64}
            library_pins = {'libz3.so': 'sha256:' + 'c' * 64}
            file_pins = {'/' + name: digest for name, digest in WORKER.SOURCES.items() if name.startswith('project/')}
        else:
            tool_pins = {'bun': WORKER.PINS['bun'], 'bendtt': WORKER.PINS['bendtt']}
            library_pins = {}
            file_pins = {'/fixtures/bend-boolean-negation-v1.bend': WORKER.SOURCES['fixtures/bend-boolean-negation-v1.bend'], '/bend2/main.ts': 'sha256:' + '1' * 64}
        file_pins.update({'/tools/' + name: digest for name, digest in tool_pins.items()})
        file_pins.update({'/tools/' + name: digest for name, digest in library_pins.items()})
        inventory = {route: {name: {'bytes': 2, 'sha256': digest} for name, digest in file_pins.items()}}
        for ordinal in range(1, 31):
            sample = {'route': route, 'ordinal': ordinal,
                      'before_reset': {name: {'pages': 1, 'resident_pages': 1} for name in inventory[route]},
                      'before_cold': {name: {'pages': 1, 'resident_pages': 0} for name in inventory[route]},
                      'before_warm': {name: {'pages': 1, 'resident_pages': 1} for name in inventory[route]}}
            for state in ('cold', 'warm'):
                sample[state] = {'exit_code': 0, 'timed_out': False, 'elapsed_ns': 12}
                body = b'app.negate proof valid' if route.startswith('semaprax') else b'ALL PROOFS CHECK'
                for channel, data in (('stdout', body), ('stderr', b'')):
                    name = f'{route}-{ordinal}-{state}-{channel}'
                    path = root / name
                    path.write_bytes(data)
                    sample[state][channel] = {'path': name, 'bytes': len(data), 'sha256': WORKER.sha(path)}
            samples.append(sample)
        source_pins = {'profile': profile, 'tools': tool_pins, 'z3_libraries': library_pins}
        if profile.startswith('native_arm64'):
            source_pins['semaprax_commit'] = CAPTURE.SEMAPRAX_COMMIT
        else:
            source_pins.update({'bend_commit': CAPTURE.BEND_COMMIT, 'bend2_files': {'main.ts': 'sha256:' + '1' * 64}})
        preflight_stdout = root / 'preflight.stdout'
        preflight_stderr = root / 'preflight.stderr'
        preflight_stdout.write_bytes(b'app.negate proof valid' if route.startswith('semaprax') else b'ALL PROOFS CHECK')
        preflight_stderr.write_bytes(b'')
        preflight = {'exit_code': 0, 'timed_out': False}
        for channel, path in (('stdout', preflight_stdout), ('stderr', preflight_stderr)):
            preflight[channel] = {'path': path.name, 'bytes': path.stat().st_size, 'sha256': WORKER.sha(path)}
        return {'schema': 'semaprax.bend2-law-benchmark.proof-verdict-cache-guest.v2',
                'profile': profile, 'route': route,
                'tool_pins': tool_pins, 'z3_library_pins': library_pins,
                'samples': samples, 'scope': WORKER.SCOPE, 'file_inventory': inventory,
                'source_pins': source_pins, 'preflight': {route: preflight}}

    def test_thirty_native_arm64_pairs_and_raw_streams_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root)
            self.assertTrue(WORKER.validate_result(result, root))
            del result['samples'][-1]
            with self.assertRaisesRegex(ValueError, 'thirty ordered pairs'):
                WORKER.validate_result(result, root)

    def test_pinned_x86_rosetta_bend_profile_keeps_its_own_thirty_pairs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root, 'pinned_x86_rosetta_bend_verdict')
            self.assertTrue(WORKER.validate_result(result, root))
            self.assertEqual({row['route'] for row in result['samples']}, {'bend_verdict'})

    def test_missing_proof_success_and_raw_stream_drift_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root)
            result['samples'][0]['cold']['exit_code'] = 1
            with self.assertRaisesRegex(ValueError, 'did not succeed'):
                WORKER.validate_result(result, root)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root)
            ref = result['samples'][0]['warm']['stderr']
            (root / ref['path']).write_bytes(b'drift')
            with self.assertRaisesRegex(ValueError, 'missing or drifted'):
                WORKER.validate_result(result, root)

    def test_every_warm_file_must_be_fully_resident_and_z3_libraries_pinned(self):
        mutations = (
            ('missing warm page', lambda value: value['samples'][0]['before_warm']['/tools/semaprax'].update(resident_pages=0)),
            ('wrong warm page count', lambda value: value['samples'][0]['before_warm']['/tools/semaprax'].update(pages=2)),
            ('unpinned z3 library', lambda value: value['file_inventory']['semaprax_z3_project_proof_check']['/tools/libz3.so'].update(sha256='sha256:' + '2' * 64)),
        )
        for name, mutate in mutations:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                result = self.result(root)
                mutate(result)
                with self.assertRaises(ValueError):
                    WORKER.validate_result(result, root)

    def test_profile_and_route_cannot_be_relabelled(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.result(Path(directory))
            result['route'] = 'bend_verdict'
            with self.assertRaisesRegex(ValueError, 'profile and fixed proof route'):
                WORKER.validate_result(result, Path(directory))


if __name__ == '__main__':
    unittest.main()
