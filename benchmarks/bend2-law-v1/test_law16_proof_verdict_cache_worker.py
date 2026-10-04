import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).parent

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

WORKER = load('proof_cache_worker', ROOT / 'law16_proof_verdict_cache_worker.py')
CAPTURE = load('proof_cache_capture', ROOT / 'law16_guest_cache_proof.py')
SOURCE_DIGEST = 'sha256:bc71b8bde8cb43cc10b49742e7e5abb3717cd28402a1ab53e7d549f540093063'

def sha(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()

class ProofVerdictCacheWorkerTests(unittest.TestCase):
    def test_arm_manifest_accepts_static_z3_and_requires_independent_sha(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'pins.json'
            value = {'schema': CAPTURE.PIN_SCHEMA, 'profile': 'native_arm64_z3_source_obligation',
                     'tools': {'z3': sha(b'z3')}, 'z3_libraries': {},
                     'image': 'docker.io/library/python@sha256:' + 'd' * 64,
                     'image_sha256': 'sha256:' + 'e' * 64,
                     'obligation': {'path': 'inputs/obligation.smt2', 'bytes': 1, 'sha256': sha(b'x')},
                                          'source_script': {'path': 'inputs/renderer.rs', 'bytes': 1, 'sha256': sha(b'x')},
                     'source': {'path': 'inputs/app.spx', 'bytes': 1, 'sha256': SOURCE_DIGEST},
                     'source_commit': 'cdfc0cdd27aa248951134f70a71bf7d36b4798de', 'renderer_version': 'test-renderer-v1'}
            path.write_bytes((json.dumps(value, separators=(',', ':'), sort_keys=True) + '\n').encode())
            self.assertEqual(CAPTURE.read_arm64_pins(path, CAPTURE.digest(path)), value)
            with self.assertRaisesRegex(ValueError, 'independently approved'):
                CAPTURE.read_arm64_pins(path, 'sha256:' + '0' * 64)
            value['z3_libraries'] = None
            path.write_text(json.dumps(value, separators=(',', ':'), sort_keys=True) + '\n')
            with self.assertRaisesRegex(ValueError, 'pin inventory'):
                CAPTURE.read_arm64_pins(path, CAPTURE.digest(path))

    def test_retained_source_and_helper_bind_stripped_smt_obligation(self):
        script = b'(assert false)\n(check-sat)\n'
        source = b'fixed source fixture'
        helper = b'pinned renderer helper'
        pins = {'obligation': {'sha256': sha(script)}, 'source': {'sha256': sha(source)},
                'source_script': {'sha256': sha(helper)}, 'source_commit': 'cdfc0cdd27aa248951134f70a71bf7d36b4798de'}
        self.assertTrue(CAPTURE.validate_source_obligation_bytes(pins, script, source, helper))
        with self.assertRaisesRegex(ValueError, 'differs'):
            CAPTURE.validate_source_obligation_bytes(pins, script + b'(get-model)\n', source, helper)

    def test_z3_success_requires_unsat_with_exit_zero(self):
        self.assertTrue(WORKER.z3_success({'exit_code': 0, 'timed_out': False}, b'unsat\n'))
        for row, out in [({'exit_code': 1}, b'unsat\n'), ({'exit_code': 0}, b'sat\n'),
                         ({'exit_code': 0}, b'unsat\n(error \"unexpected\")\n'), ({'exit_code': 0, 'timed_out': True}, b'unsat\n')]:
            self.assertFalse(WORKER.z3_success(row, out))

    def test_cli_help_has_unique_pin_digest_option_and_arm_arguments(self):
        with patch('sys.stdout'):
            with self.assertRaises(SystemExit) as caught:
                CAPTURE.main(['--help'])
        self.assertEqual(caught.exception.code, 0)

    def test_python_image_must_match_architecture_and_document_python3(self):
        image = [{'variants': [{'platform': {'architecture': 'arm64'}, 'config': {'history': [{'created_by': 'RUN install python3'}]}}]}]
        CAPTURE.verify_python_image(image, 'arm64')
        with self.assertRaisesRegex(ValueError, 'does not document'):
            CAPTURE.verify_python_image([{'variants': [{'platform': {'architecture': 'arm64'}, 'config': {'history': []}}]}], 'arm64')

    def test_zero_byte_inventory_file_has_empty_residency_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'empty'
            path.touch()
            self.assertEqual(WORKER.resident(path), {'pages': 0, 'resident_pages': 0})

    def result(self, root, profile='native_arm64_z3_source_obligation'):
        route = WORKER.PROFILES[profile]['route']
        is_arm = profile.startswith('native_arm64')
        tool_pins = {'z3': WORKER.PINS['z3']} if is_arm else {'bun': WORKER.PINS['bun'], 'bendtt': WORKER.PINS['bendtt']}
        library_pins = {}
        inventory_paths = {'/opt/law16/tools/' + name: digest for name, digest in tool_pins.items()}
        if is_arm:
            pins = {'obligation': {'sha256': sha(b'obligation')}, 'source_script': {'sha256': sha(b'renderer')}, 'source': {'sha256': SOURCE_DIGEST}}
            inventory_paths.update({'/opt/law16/obligation.smt2': pins['obligation']['sha256'], '/opt/law16/source-script': pins['source_script']['sha256'], '/opt/law16/source/src/app.spx': SOURCE_DIGEST})
        else:
            pins = {'bend_commit': CAPTURE.BEND_COMMIT, 'bend2_files': {'main.ts': sha(b'bend')}}
            inventory_paths.update({'/opt/law16/fixtures/bend-boolean-negation-v1.bend': WORKER.SOURCES['fixtures/bend-boolean-negation-v1.bend'], '/opt/law16/bend2/main.ts': sha(b'bend')})
        inventory = {route: {name: {'bytes': 1, 'sha256': digest, 'mode': 0o755 if name.startswith('/opt/law16/tools/') else 0o644} for name, digest in inventory_paths.items()}}
        samples = []
        for ordinal in range(1, 31):
            sample = {'route': route, 'ordinal': ordinal,
                      'before_reset': {name: {'pages': 1, 'resident_pages': 1} for name in inventory[route]},
                      'before_cold': {name: {'pages': 1, 'resident_pages': 0} for name in inventory[route]},
                      'before_warm': {name: {'pages': 1, 'resident_pages': 1} for name in inventory[route]}}
            for state in ('cold', 'warm'):
                stdout = b'unsat\n' if is_arm else b'ALL PROOFS CHECK'
                sample[state] = {'exit_code': 0, 'timed_out': False, 'elapsed_ns': 12}
                for channel, data in (('stdout', stdout), ('stderr', b'')):
                    name = f'{route}-{ordinal}-{state}-{channel}'
                    (root / name).write_bytes(data)
                    sample[state][channel] = {'path': name, 'bytes': len(data), 'sha256': WORKER.sha(root / name)}
            samples.append(sample)
        preflight_stdout = b'unsat\n' if is_arm else b'ALL PROOFS CHECK'
        (root / 'preflight.stdout').write_bytes(preflight_stdout)
        (root / 'preflight.stderr').write_bytes(b'')
        preflight = {'exit_code': 0, 'timed_out': False}
        for channel in ('stdout', 'stderr'):
            path = root / ('preflight.' + channel)
            preflight[channel] = {'path': path.name, 'bytes': path.stat().st_size, 'sha256': WORKER.sha(path)}
        return {'schema': 'semaprax.bend2-law-benchmark.proof-verdict-cache-guest.v2', 'profile': profile, 'route': route,
                'tool_pins': tool_pins, 'z3_library_pins': library_pins, 'source_pins': {'profile': profile, 'tools': tool_pins, 'z3_libraries': library_pins, **pins},
                'samples': samples, 'scope': WORKER.SCOPE, 'file_inventory': inventory, 'preflight': {route: preflight}}

    def test_thirty_arm_source_obligation_pairs_and_raw_streams_required(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root)
            self.assertTrue(WORKER.validate_result(result, root))
            del result['samples'][-1]
            with self.assertRaisesRegex(ValueError, 'thirty ordered pairs'):
                WORKER.validate_result(result, root)

    def test_x86_bend_profile_retains_separate_thirty_pairs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root, 'pinned_x86_rosetta_bend_verdict')
            self.assertTrue(WORKER.validate_result(result, root))
            self.assertEqual({sample['route'] for sample in result['samples']}, {'bend_verdict'})

    def test_arm_inventory_rejects_missing_obligation_or_wrong_residency(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.result(root)
            result['file_inventory']['z3_source_obligation'].pop('/opt/law16/obligation.smt2')
            with self.assertRaises(ValueError):
                WORKER.validate_result(result, root)

if __name__ == '__main__':
    unittest.main()
