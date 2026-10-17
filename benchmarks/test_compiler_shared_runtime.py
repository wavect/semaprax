"""Pure hostile protocol checks. No app-server/compiler/provider process is run."""
from __future__ import annotations
import base64
import copy
import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import compiler_execution_broker as broker
import compiler_shared_runtime as runtime
from compiler_runtime_protocol import Protocol, Refusal, decode, encode


class SharedRuntimeChecks(unittest.TestCase):
    def setUp(self):
        self.protocol = Protocol()

    def config(self):
        return {'backend': runtime.BACKEND, 'model': 'test-model-not-executed',
            'effort': 'medium', 'arms': ['semaprax', 'typescript'],
            'runtime_contract_sha256': 'a' * 64, 'compiler_tool_arms': ['semaprax'],
            'allow_model_fallback': False}

    def test_matched_backend_is_explicit_and_legacy_labels_refuse(self):
        good = self.config()
        self.assertEqual(runtime.matched_configuration(good), good)
        for field, value in [('backend', 'codex-exec'), ('arms', ['semaprax']),
                ('allow_model_fallback', True), ('compiler_tool_arms', ['typescript']),
                ('runtime_contract_sha256', 'z' * 64)]:
            with self.subTest(field=field):
                bad = dict(good, **{field: value})
                self.assertRaises(Refusal, runtime.matched_configuration, bad)
        self.assertRaises(Refusal, runtime.matched_configuration, dict(good, qualified=True))

    def test_caller_dict_cannot_create_live_authority_or_start_a_process(self):
        with mock.patch.object(runtime.subprocess, 'Popen', side_effect=AssertionError('no process')):
            with tempfile.TemporaryDirectory(prefix='869-offline-') as tmp:
                self.assertRaises(Refusal, runtime.Session, {'qualified': True}, Path(tmp) / 'private', self.config())

    def test_duplicate_unknown_and_malformed_frames_refuse(self):
        for raw in [b'{"id":1,"id":1}', b'{"id":NaN}', b'\xff', b'{' , b' ' * (1024 * 1024 + 1)]:
            self.assertRaises(Refusal, decode, raw)
        command = {'command': ['/protected/compiler'], 'cwd': '/workspace',
            'sandboxPolicy': {'type': 'workspaceWrite', 'writableRoots': ['/workspace'],
                'networkAccess': False, 'excludeSlashTmp': True, 'excludeTmpdirEnvVar': True},
            'env': {'DYLD_INSERT_LIBRARIES': None}, 'processId': 'offline',
            'streamStdoutStderr': True, 'outputBytesCap': 513, 'timeoutMs': 1000}
        self.protocol.validate('command/exec', command)
        self.assertRaises(Refusal, self.protocol.validate, 'command/exec', dict(command, guessedField=True))
        bad = copy.deepcopy(command)
        bad['sandboxPolicy']['foreignPolicy'] = True
        self.assertRaises(Refusal, self.protocol.validate, 'command/exec', bad)
        self.assertRaises(Refusal, self.protocol.validate, 'process/spawn', command)

    def test_real_installed_truncation_shape_and_foreign_stream_refuse(self):
        session = object.__new__(runtime.Session)
        session.command = {'process_id': 'owned', 'stdout': bytearray(), 'stderr': bytearray()}
        params = {'capReached': False, 'deltaBase64': base64.b64encode(b'ok\n').decode(),
            'processId': 'owned', 'stream': 'stdout'}
        notification = {'method': 'command/exec/outputDelta', 'params': params}
        self.protocol.validate('notification', notification)
        self.protocol.validate('notification', dict(notification, emittedAtMs=1))
        self.assertRaises(Refusal, self.protocol.validate, 'notification',
            dict(notification, emittedAtMs='foreign'))
        session._command_delta(params)
        self.assertEqual(session.command['stdout'], b'ok\n')
        for key, value in [('capReached', True), ('processId', 'foreign'),
                ('stream', 'unknown'), ('deltaBase64', '!!!!')]:
            self.assertRaises(Refusal, session._command_delta, dict(params, **{key: value}))
        self.assertRaises(Refusal, session._command_delta,
            dict(params, deltaBase64=base64.b64encode(b'x' * 513).decode()))
        self.assertRaises(Refusal, self.protocol.validate, 'notification',
            {'method': 'command/exec/outputDelta', 'params': {k:v for k,v in params.items() if k != 'capReached'}})

    def test_exact_complete_stdout_seal_binds_nonce_length_and_retained_bytes(self):
        artifact = b'{"retained":"exact"}\n'
        seal = {'schema': 'semaprax.json-codec-publication-seal.v1', 'nonce': 'runner-owned',
            'receipt_bytes': len(artifact), 'receipt_sha256': 'sha256:' + hashlib.sha256(artifact).hexdigest()}
        raw = encode(seal)
        self.assertEqual(broker.publication_seal(raw, 'runner-owned', artifact), seal)
        for changed in [raw[:-1], raw + raw, raw + b'garbage', raw.replace(b'runner-owned', b'foreign'),
                encode(dict(seal, receipt_bytes=True)), encode(dict(seal, receipt_bytes=len(artifact)+1)),
                encode(dict(seal, receipt_sha256='sha256:'+'0'*64)), encode(dict(seal, extra=True))]:
            self.assertRaises(Refusal, broker.publication_seal, changed, 'runner-owned', artifact)
        self.assertRaises(Refusal, broker.publication_seal, raw, 'runner-owned', artifact + b' ')

    def test_closed_request_has_no_caller_nonce_binary_policy_or_environment(self):
        authority = SimpleNamespace(workspace_root='/workspace/candidate', authorize_cwd=lambda _: None)
        request = {'cwd': '/workspace/candidate', 'argv': ['json-codec', '.', '--source', 'main.spx',
            '--type', 'record.id', '--output', 'result.json', '--retained-receipt']}
        self.assertEqual(broker._request(request, authority, Path('/workspace/candidate'))[0], tuple(request['argv']))
        for field in ['nonce', 'binary', 'policy', 'env', 'exit_code', 'stdout']:
            self.assertRaises(Refusal, broker._request, dict(request, **{field: 'forged'}), authority, Path('/workspace/candidate'))
        for argv in [request['argv'] + ['--receipt-stdout-seal', 'attacker'],
                ['/foreign/compiler', *request['argv']], ['sh', '-c', 'echo fake']]:
            self.assertRaises(Refusal, broker._request, dict(request, argv=argv), authority, Path('/workspace/candidate'))
        self.assertRaises(Refusal, broker._request, dict(request, cwd='/foreign'), authority, Path('/workspace/candidate'))
        escaped = request['argv'][:]
        escaped[escaped.index('--output')+1] = '../foreign'
        self.assertRaises(ValueError, broker._request, dict(request, argv=escaped), authority, Path('/workspace/candidate'))

    def test_selected_actual_response_id_and_replay_are_closed(self):
        session = object.__new__(runtime.Session)
        session.protocol = self.protocol
        session.pending = {7}
        session.responses = {}
        session._dispatch({'id': 7, 'result': {'exitCode': 0, 'stdout': '', 'stderr': ''}})
        self.assertIn(7, session.responses)
        for value in [{'id': 7, 'result': {}}, {'id': 8, 'result': {}},
                {'id': True, 'result': {}}, {'id': 7, 'result': {}, 'error': {}},
                {'id': 7, 'result': {}, 'forged': 'policy'}]:
            self.assertRaises(Refusal, session._dispatch, value)

    def test_entire_original_structural_inventory_kernel_is_retained(self):
        old = Path(broker.retained.__file__).read_text()
        start = old.index('    receipt=_object(_decode(raw)')
        end = old.index("    return {'schema':'semaprax.compiler-retained-receipt-observation.v1'")
        self.assertIn(old[start:end], Path(broker.__file__).read_text())
        self.assertNotIn("'observer': 'sandboxed_harness_proxy'", Path(broker.__file__).read_text())


if __name__ == '__main__':
    unittest.main()
