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
            'runtime_contracts_sha256': {'semaprax': 'a' * 64, 'typescript': 'd' * 64},
            'runtime_sha256': 'b' * 64, 'compiler_sha256': 'c' * 64, 'compiler_tool_arms': ['semaprax'],
            'allow_model_fallback': False}

    def test_matched_backend_is_explicit_and_legacy_labels_refuse(self):
        good = self.config()
        self.assertEqual(runtime.matched_configuration(good), good)
        for field, value in [('backend', 'codex-exec'), ('arms', ['semaprax']),
                ('allow_model_fallback', True), ('compiler_tool_arms', ['typescript']),
                ('runtime_sha256', 'z' * 64)]:
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

    def test_authority_scope_rejects_valid_but_wider_installed_protocol_requests(self):
        # Static plan-shaped metadata only: this does not construct a grant,
        # initialize a Session, or run any RPC.
        session = object.__new__(runtime.Session)
        session.authority = SimpleNamespace(workspace_root=Path('/candidate'),
            compiler_binary=Path('/protected/compiler'), command_env={'LANG': 'C'},
            sandbox_policy={'type': 'workspaceWrite', 'writableRoots': ('/candidate',),
                'networkAccess': False, 'excludeSlashTmp': True, 'excludeTmpdirEnvVar': True})
        session.config = self.config()
        session.thread_id = 'owned-thread'
        session.declared_tools = []
        session.command = {'process_id': 'owned-process'}
        command = {'command': ['/protected/compiler', 'json-codec', '.', '--source', 'schema.spx',
                '--type', 'record.id', '--output', 'receipt.json', '--retained-receipt',
                '--receipt-stdout-seal', 'a' * 64],
            'cwd': '/candidate', 'env': {'LANG': 'C'},
            'sandboxPolicy': {'type': 'workspaceWrite', 'writableRoots': ['/candidate'],
                'networkAccess': False, 'excludeSlashTmp': True, 'excludeTmpdirEnvVar': True},
            'processId': 'owned-process', 'tty': False, 'streamStdin': False,
            'streamStdoutStderr': True, 'outputBytesCap': 513, 'timeoutMs': 1000}
        self.protocol.validate('command/exec', command)
        session._authorize_parameters('command/exec', command)
        attacks = []
        for key, value in [('cwd', '/foreign'), ('env', {'LANG': 'C', 'DYLD_INSERT_LIBRARIES': '/evil'}),
                ('sandboxPolicy', {'type': 'dangerFullAccess'}), ('processId', 'foreign'),
                ('tty', True), ('streamStdin', True), ('streamStdoutStderr', False),
                ('outputBytesCap', 512), ('timeoutMs', 0), ('timeoutMs', 60001)]:
            attacks.append(dict(command, **{key: value}))
        for argv in [['/protected/compiler', 'project', 'repair'],
                ['/foreign/compiler', *command['command'][1:]],
                [*command['command'][:-1], 'foreign_nonce']]:
            attacks.append(dict(command, command=argv))
        for attack in attacks:
            self.assertRaises(Refusal, session._authorize_parameters, 'command/exec', attack)
        self.assertRaises(Refusal, session._authorize_parameters, 'process/spawn', command)
        thread = {'cwd': '/candidate', 'runtimeWorkspaceRoots': ['/candidate'], 'environments': [],
            'sandbox': 'workspace-write', 'approvalPolicy': 'never', 'model': self.config()['model'],
            'allowProviderModelFallback': False, 'config': {'model_reasoning_effort': 'medium'},
            'dynamicTools': []}
        session._authorize_parameters('thread/start', thread)
        for key, value in [('approvalPolicy', 'onRequest'), ('environments', [{'environmentId': 'foreign'}]),
                ('config', {'model_reasoning_effort': 'medium', 'sandbox_mode': 'danger-full-access'}),
                ('dynamicTools', [{'name': 'foreign'}]), ('runtimeWorkspaceRoots', ['/'])]:
            self.assertRaises(Refusal, session._authorize_parameters, 'thread/start', dict(thread, **{key:value}))

    def test_dynamic_request_binding_duplicate_ids_and_call_replay_refuse(self):
        session = object.__new__(runtime.Session)
        session.protocol = self.protocol
        session.thread_id, session.turn_id = 'thread', 'turn'
        session.server_ids, session.call_ids = set(), set()
        session.deferred_requests = []
        sent, handled = [], []
        session.send = sent.append
        session.handler = lambda value: (handled.append(value) or {'success': False,
            'contentItems': [{'type': 'inputText', 'text': 'synthetic refusal; no execution'}]})
        message = {'method': 'item/tool/call', 'id': 30, 'params': {'arguments': {'synthetic': True},
            'callId': 'one-call', 'threadId': 'thread', 'turnId': 'turn', 'tool': runtime.TOOL}}
        session._dispatch(message)
        self.assertEqual(len(handled), 1)
        self.assertEqual(sent[0]['id'], 30)
        self.assertRaises(Refusal, session._dispatch, message)
        self.assertRaises(Refusal, session._dispatch, dict(message, id=31))
        foreign = copy.deepcopy(message)
        foreign['id'] = 32
        foreign['params'].update(callId='foreign-call', threadId='foreign-thread')
        self.assertRaises(Refusal, session._dispatch, foreign)
        self.assertEqual(len(handled), 1)

    def test_protocol_integer_overflow_and_nonfinite_exponent_are_refused(self):
        for value in [True, 2**31, -(2**31)-1]:
            self.assertRaises(Refusal, self.protocol.validate, 'command_result',
                {'exitCode': value, 'stdout': '', 'stderr': ''})
        self.assertRaises(Refusal, decode, b'{"value":1e9999}')

    def test_complete_old_inventory_kernel_validates_synthetic_success_and_hostile_receipts(self):
        from test_compiler_retained_receipt import CompilerRetainedReceiptTests
        old = CompilerRetainedReceiptTests()
        with tempfile.TemporaryDirectory(prefix='869-structure-only-') as tmp:
            fixture = old.fixture(Path(tmp))
            candidate, compiler, argv, document, selected, _ = fixture
            original = old.validate(fixture)
            current = broker.validate_trusted_receipt(old.raw(document), argv, str(candidate), compiler, selected)
            self.assertEqual(current['inventories'], original['inventories'])
            self.assertFalse(current['ratio_eligible'])
            self.assertIsNone(current['actual_billed_usd'])
            self.assertNotIn('execution_independently_authenticated', current)
            for kind in range(9):
                bad = copy.deepcopy(document)
                if kind == 0: bad['held_inputs'].pop()
                elif kind == 1: bad['held_inputs'][1]['source'] += 'foreign'
                elif kind == 2: bad['derivations'].pop()
                elif kind == 3: bad['derivations'][1]['bundle_sha256'] = 'sha256:' + '0'*64
                elif kind == 4: bad['bundle']['entries'][0]['record_id'] = 'foreign'
                elif kind == 5: bad['independent_execution'] = True
                elif kind == 6: bad['bundle']['candidate_sources'][0]['source'] += 'foreign'
                elif kind == 7: bad['bundle']['candidate_sources'].pop()
                else: bad['unknown_origin_field'] = True
                self.assertRaises(ValueError, broker.validate_trusted_receipt,
                    old.raw(bad), argv, str(candidate), compiler, selected)

    def test_timeout_eof_and_oversized_runtime_frames_refuse_without_a_runtime(self):
        session = object.__new__(runtime.Session)
        session.buffer = bytearray()
        self.assertRaises(Refusal, session._next, 0)
        session.buffer = bytearray(b'x' * (1024 * 1024 + 1))
        self.assertRaises(Refusal, session._next, float('inf'))
        session.buffer = bytearray()
        stdout = object()
        session.process = SimpleNamespace(stdout=stdout)
        session.selector = SimpleNamespace(select=lambda _: [(SimpleNamespace(fd=-1, fileobj=stdout), 1)],
            unregister=lambda _: None)
        with mock.patch.object(runtime.os, 'read', return_value=b''):
            self.assertRaises(Refusal, session._next, float('inf'))

    def test_owned_group_timeout_escalates_and_all_local_handles_close(self):
        # In-memory process interface only; os.killpg is mocked, no process starts.
        class Pipe:
            def __init__(self): self.closed = False
            def close(self): self.closed = True
        session = object.__new__(runtime.Session)
        session.closed = False
        pipes = [Pipe() for _ in range(5)]
        session.process = SimpleNamespace(pid=987654321, stdin=pipes[0], stdout=pipes[1], stderr=pipes[2],
            wait=mock.Mock(side_effect=[runtime.subprocess.TimeoutExpired('synthetic', 2), 0]))
        session.selector = Pipe()
        session.transcript, session.stderr = pipes[3:]
        calls = []
        def kill(pid, sig):
            calls.append((pid, sig))
            if sig == 0: raise ProcessLookupError()
        with mock.patch.object(runtime.os, 'killpg', side_effect=kill):
            session.close()
            session.close()
        self.assertEqual(calls, [(987654321, runtime.signal.SIGTERM),
            (987654321, runtime.signal.SIGKILL), (987654321, 0)])
        self.assertTrue(all(pipe.closed for pipe in pipes))
        self.assertTrue(session.selector.closed)

    def test_direct_request_ingress_cannot_skip_authority_or_mutate_request_state(self):
        session = object.__new__(runtime.Session)
        session.authority = {'qualified': True}
        session.sequence, session.pending = 4, set()
        sent = []
        session.send = sent.append
        self.assertRaises(Refusal, session.request, 'command/exec', {}, 1)
        self.assertEqual(sent, [])
        self.assertEqual(session.sequence, 4)
        self.assertEqual(session.pending, set())

    def test_matched_configuration_binds_both_distinct_trial_contracts_and_fixed_policy(self):
        config = self.config()
        self.assertNotEqual(config['runtime_contracts_sha256']['semaprax'],
                            config['runtime_contracts_sha256']['typescript'])
        for contracts in [{'semaprax': 'a'*64}, {'semaprax':'a'*64,'typescript':'bad'},
                {'semaprax':'a'*64,'typescript':'d'*64,'foreign':'f'*64}]:
            self.assertRaises(Refusal, runtime.matched_configuration,
                dict(config, runtime_contracts_sha256=contracts))
        authority = SimpleNamespace(workspace_root=Path('/candidate'), sandbox_policy={
            'type': 'workspaceWrite', 'writableRoots': ('/candidate',), 'networkAccess': False,
            'excludeSlashTmp': True, 'excludeTmpdirEnvVar': True})
        self.assertEqual(runtime.fixed_policy(authority)['writableRoots'], ['/candidate'])
        for key, value in [('writableRoots', ['/candidate','/foreign']), ('networkAccess', True),
                ('excludeSlashTmp', False), ('excludeTmpdirEnvVar', False)]:
            self.assertRaises(Refusal, runtime.fixed_policy, SimpleNamespace(
                workspace_root=Path('/candidate'), sandbox_policy=dict(authority.sandbox_policy, **{key:value})))

    def test_evidence_collision_does_not_open_or_leak_a_candidate_descriptor(self):
        with tempfile.TemporaryDirectory(prefix='869-resource-only-') as tmp:
            root = Path(tmp)
            evidence = root / 'evidence'
            evidence.mkdir()
            with mock.patch.object(broker.os, 'open', side_effect=AssertionError('must not open')) as opened:
                self.assertRaises(FileExistsError, broker._root_and_evidence, root, evidence)
                opened.assert_not_called()
            fresh = root / 'fresh'
            self.assertRaises(FileNotFoundError, broker._root_and_evidence, root / 'absent', fresh)
            self.assertFalse(fresh.exists())

    def test_turn_wall_bound_cannot_be_disabled_or_made_unbounded(self):
        session = object.__new__(runtime.Session)
        for timeout in [0, -1, 1801, float('inf'), float('nan'), True, 'unlimited']:
            with self.assertRaisesRegex(Refusal, 'runtime turn wall bound differs'):
                session.run_turn('not sent', None, timeout)


if __name__ == '__main__':
    unittest.main()
