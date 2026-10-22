"""Synthetic Session transport controls, never effective-policy qualification.

Only require_authority is mocked. Real Session request/dispatch/result handling
and channel custody run against in-memory frames; no process/RPC/provider runs.
Imported by the existing test_compiler_shared_runtime owning unittest harness.
"""
from __future__ import annotations
import base64
import copy
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest import mock

import compiler_executor_channel as channel
import compiler_shared_runtime as runtime
from compiler_runtime_protocol import Protocol, Refusal


class CompletionChannelChecks(unittest.TestCase):
    def session(self, status=0, output=b'synthetic-seal\n', attack=None):
        session = object.__new__(runtime.Session)
        session.initialized, session.closed = True, False
        session.authority = SimpleNamespace(
            workspace_root=Path('/candidate'), compiler_binary=Path('/protected/compiler'),
            contract_sha256='a' * 64, runtime_sha256='b' * 64, compiler_sha256='c' * 64,
            command_env={'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC'},
            sandbox_policy={'type': 'workspaceWrite', 'writableRoots': ['/candidate'],
                'networkAccess': False, 'excludeSlashTmp': True, 'excludeTmpdirEnvVar': True},
            authorize_cwd=lambda cwd: self.assertEqual(cwd, '/candidate'))
        session.protocol, session.sequence = Protocol(), 0
        session.pending, session.responses, session.events = set(), {}, []
        session.command = None
        frames, sent = [], []

        def send(request):
            sent.append(copy.deepcopy(request))
            params = request['params']
            frames.extend([
                {'method': 'command/exec/outputDelta', 'params': {'capReached': False,
                    'deltaBase64': base64.b64encode(output).decode(),
                    'processId': params['processId'], 'stream': 'stdout'}},
                {'id': request['id'], 'result': {'exitCode': status, 'stdout': '', 'stderr': ''}},
            ])
            if attack is not None:
                attack(frames)

        def next_frame(_deadline):
            if not frames:
                raise Refusal('synthetic runtime EOF before complete response')
            return frames.pop(0)

        session.send = send
        session._next = next_frame
        session.close = mock.Mock(side_effect=lambda: setattr(session, 'closed', True))
        return session, sent

    def argv(self):
        return ('/protected/compiler', 'json-codec', '.', '--source', 'main.spx',
            '--type', 'record.id', '--output', 'result.json', '--retained-receipt',
            '--receipt-stdout-seal', 'd' * 64)

    def execute(self, session, owned):
        return session.execute_owned(owned, self.argv(), '/candidate')

    def test_actual_session_request_dispatch_binds_one_immutable_completion(self):
        with mock.patch.object(runtime, 'require_authority'):
            session, sent = self.session()
            owned = channel.CompletionChannel(session)
            token = self.execute(session, owned)
            self.assertIs(type(token), channel.Completion)
            self.assertIsNone(session.command)
            self.assertEqual(len(sent), 1)
            result = owned.consume(session, token, self.argv(), '/candidate')
            self.assertEqual(result['request_id'], sent[0]['id'])
            self.assertEqual(result['process_id'], sent[0]['params']['processId'])
            self.assertEqual(result['argv'], self.argv())
            self.assertEqual(result['stdout'], b'synthetic-seal\n')
            self.assertEqual(result['exit_code'], 0)
            self.assertEqual(result['runtime_contract_sha256'], 'a' * 64)
            self.assertEqual(result['completion_binding_schema'], channel.SCHEMA)
            self.assertEqual(len(result['completion_binding_sha256']), 64)
            with self.assertRaises(TypeError):
                result['exit_code'] = 99
            self.assertRaises(Refusal, owned.consume, session, token, self.argv(), '/candidate')
            self.assertRaises(Refusal, channel.Completion)
            self.assertFalse(session.closed)

    def test_caller_dict_foreign_token_and_incomplete_command_cannot_complete(self):
        with mock.patch.object(runtime, 'require_authority'):
            for fake in [{}, {'exit_code': 0}, object(), object.__new__(channel.Completion)]:
                session, sent = self.session()
                owned = channel.CompletionChannel(session)
                token = self.execute(session, owned)
                self.assertRaises(Refusal, owned.consume, session, fake, self.argv(), '/candidate')
                self.assertRaises(Refusal, owned.consume, session, token, self.argv(), '/candidate')
                self.assertEqual(len(sent), 1)
            session, sent = self.session()
            owned = channel.CompletionChannel(session)
            # No Session command / no matching future request is not a producer.
            self.assertRaises(Refusal, owned._begin, session, self.argv(), '/candidate', 1, 'e' * 32)
            session.command = {'process_id': 'e' * 32, 'stdout': bytearray(), 'stderr': bytearray()}
            ticket = owned._begin(session, self.argv(), '/candidate', 1, 'e' * 32)
            # Even a pending ticket has no accepted response. No result dict is
            # accepted as an argument to _complete, and no completion publishes.
            self.assertRaises(Refusal, owned._complete, session, ticket)
            self.assertIsNone(owned._active)
            self.assertEqual(sent, [])

    def test_exact_session_and_channel_owners_reject_foreign_adoption(self):
        with mock.patch.object(runtime, 'require_authority'):
            session, sent = self.session()
            other, other_sent = self.session()
            owned, foreign = channel.CompletionChannel(session), channel.CompletionChannel(other)
            token = self.execute(session, owned)
            self.assertRaises(Refusal, owned.consume, other, token, self.argv(), '/candidate')
            self.assertRaises(Refusal, foreign.consume, other, token, self.argv(), '/candidate')
            self.assertRaises(Refusal, other.execute_owned, owned, self.argv(), '/candidate')
            self.assertEqual(owned.consume(session, token, self.argv(), '/candidate')['exit_code'], 0)
            self.assertEqual(len(sent), 1)
            self.assertEqual(other_sent, [])
            self.assertRaises(Refusal, channel.CompletionChannel, SimpleNamespace(initialized=True, closed=False))

    def test_invocation_policy_environment_contract_and_authority_drift_burn_completion(self):
        with mock.patch.object(runtime, 'require_authority'):
            for attack in ['argv', 'cwd', 'contract', 'runtime', 'compiler', 'binary',
                           'environment', 'policy', 'authority', 'closed']:
                with self.subTest(attack=attack):
                    session, sent = self.session()
                    owned = channel.CompletionChannel(session)
                    token = self.execute(session, owned)
                    argv, cwd = self.argv(), '/candidate'
                    if attack == 'argv': argv = (*argv[:-1], 'f' * 64)
                    elif attack == 'cwd': cwd = '/foreign'
                    elif attack == 'contract': session.authority.contract_sha256 = 'f' * 64
                    elif attack == 'runtime': session.authority.runtime_sha256 = 'f' * 64
                    elif attack == 'compiler': session.authority.compiler_sha256 = 'f' * 64
                    elif attack == 'binary': session.authority.compiler_binary = Path('/foreign/compiler')
                    elif attack == 'environment': session.authority.command_env['LANG'] = 'foreign'
                    elif attack == 'policy': session.authority.sandbox_policy['networkAccess'] = True
                    elif attack == 'authority': session.authority = copy.copy(session.authority)
                    else: session.closed = True
                    self.assertRaises(Refusal, owned.consume, session, token, argv, cwd)
                    self.assertRaises(Refusal, owned.consume, session, token, self.argv(), '/candidate')
                    self.assertEqual(len(sent), 1)

    def test_strict_immutable_completion_fields_cannot_use_equal_subclasses(self):
        class EqualString(str):
            pass
        with mock.patch.object(runtime, 'require_authority'):
            for field in ['process_id', 'cwd', 'runtime_contract_sha256', 'argv',
                          'request_id', 'exit_code', 'stdout', 'stderr']:
                with self.subTest(field=field):
                    session, sent = self.session()
                    owned = channel.CompletionChannel(session)
                    complete = owned._complete
                    def tamper(owner, token):
                        result = owner.command['completion']
                        if field in ['process_id', 'cwd', 'runtime_contract_sha256']:
                            result[field] = EqualString(result[field])
                        elif field == 'argv': result[field] = (EqualString(result[field][0]), *result[field][1:])
                        elif field in ['request_id', 'exit_code']: result[field] = True
                        else: result[field] = bytearray(result[field])
                        return complete(owner, token)
                    with mock.patch.object(owned, '_complete', side_effect=tamper):
                        self.assertRaises(Refusal, self.execute, session, owned)
                    self.assertIsNone(owned._active)
                    self.assertTrue(session.closed)
                    self.assertEqual(len(sent), 1)

    def test_result_copy_and_mid_request_owner_drift_cannot_mutate_or_publish(self):
        with mock.patch.object(runtime, 'require_authority'):
            session, _ = self.session()
            owned = channel.CompletionChannel(session)
            held = []
            complete = owned._complete
            def capture(owner, token):
                held.append(owner.command['completion'])
                return complete(owner, token)
            with mock.patch.object(owned, '_complete', side_effect=capture):
                token = self.execute(session, owned)
            held[0]['exit_code'] = 99
            held[0]['stdout'] = b'forged after completion'
            result = owned.consume(session, token, self.argv(), '/candidate')
            self.assertEqual(result['exit_code'], 0)
            self.assertEqual(result['stdout'], b'synthetic-seal\n')
            for attack in ['authority', 'command', 'request', 'process']:
                session, sent = self.session()
                owned = channel.CompletionChannel(session)
                complete = owned._complete
                def drift(owner, token):
                    if attack == 'authority': owner.authority = copy.copy(owner.authority)
                    elif attack == 'command': owner.command = dict(owner.command)
                    elif attack == 'request': owner.sequence += 1
                    else: owner.command['process_id'] = 'foreign'
                    return complete(owner, token)
                with mock.patch.object(owned, '_complete', side_effect=drift):
                    self.assertRaises(Refusal, self.execute, session, owned)
                self.assertIsNone(owned._active)
                self.assertIsNone(session.command)
                self.assertTrue(session.closed)
                self.assertEqual(len(sent), 1)

    def test_utf8_hash_and_argv_shape_refuse_before_command_send(self):
        with mock.patch.object(runtime, 'require_authority'):
            for attack in ['surrogate', 'cwd-surrogate', 'compiler-sha', 'runtime-sha',
                           'generator', 'list', 'bool-timeout']:
                with self.subTest(attack=attack):
                    session, sent = self.session()
                    owned = channel.CompletionChannel(session)
                    argv, cwd, timeout = self.argv(), '/candidate', 60
                    if attack == 'surrogate': argv = (*argv[:-1], '\ud800')
                    elif attack == 'cwd-surrogate':
                        cwd = '/candidate\ud800'
                        session.authority.authorize_cwd = lambda _cwd: None
                    elif attack == 'compiler-sha': session.authority.compiler_sha256 = 'not-a-sha'
                    elif attack == 'runtime-sha': session.authority.runtime_sha256 = 'z' * 64
                    elif attack == 'generator': argv = iter(argv)
                    elif attack == 'list': argv = list(argv)
                    else: timeout = True
                    self.assertRaises(Refusal, session.execute_owned, owned, argv, cwd, timeout)
                    self.assertEqual(sent, [])
            session, _ = self.session()
            owned = channel.CompletionChannel(session)
            token = self.execute(session, owned)
            self.assertRaises(Refusal, owned.consume, session, token, iter(self.argv()), '/candidate')
            self.assertRaises(Refusal, owned.consume, session, token, self.argv(), '/candidate')

    def test_wrong_response_process_truncation_and_transport_abort_never_adopt(self):
        def bad_id(frames): frames[-1]['id'] += 1
        def bad_process(frames): frames[0]['params']['processId'] = 'foreign'
        def truncated(frames): frames[0]['params']['capReached'] = True
        def stdout_result(frames): frames[-1]['result']['stdout'] = 'forged'
        def foreign_exit(frames): frames[-1]['result']['exitCode'] = True
        def error_result(frames): frames[-1] = {'id': frames[-1]['id'], 'error': {'code': 1, 'message': 'refused'}}
        def missing_result(frames): frames.pop()
        with mock.patch.object(runtime, 'require_authority'):
            for attack in [bad_id, bad_process, truncated, stdout_result, foreign_exit, error_result, missing_result]:
                with self.subTest(attack=attack.__name__):
                    session, sent = self.session(attack=attack)
                    owned = channel.CompletionChannel(session)
                    self.assertRaises(Refusal, self.execute, session, owned)
                    self.assertTrue(session.closed)
                    self.assertIsNone(owned._active)
                    self.assertIsNone(session.command)
                    self.assertEqual(len(sent), 1)
            for failure in [Refusal('synthetic timeout'), TimeoutError('synthetic kill'), KeyboardInterrupt()]:
                session, sent = self.session()
                owned = channel.CompletionChannel(session)
                session._next = mock.Mock(side_effect=failure)
                self.assertRaises(type(failure), self.execute, session, owned)
                self.assertTrue(session.closed)
                self.assertIsNone(owned._active)
                self.assertEqual(len(sent), 1)

    def test_request_process_replay_and_command_bound_refuse_without_retry(self):
        with mock.patch.object(runtime, 'require_authority'):
            for attack in ['request', 'process', 'limit']:
                session, sent = self.session()
                owned = channel.CompletionChannel(session)
                with mock.patch.object(runtime.uuid, 'uuid4', return_value=SimpleNamespace(hex='e' * 32)):
                    token = self.execute(session, owned)
                    owned.consume(session, token, self.argv(), '/candidate')
                    if attack == 'request': session.sequence = 0
                    elif attack == 'limit': owned._processes = {format(i, '032x') for i in range(channel.MAX_COMMANDS)}
                    self.assertRaises(Refusal, self.execute, session, owned)
                self.assertTrue(session.closed)
                self.assertIsNone(owned._active)
                self.assertEqual(len(sent), 1)

    def test_legacy_plain_execute_remains_plain_and_authority_still_refuses(self):
        with mock.patch.object(runtime, 'require_authority'):
            session, sent = self.session(status=17)
            result = session.execute(list(self.argv()), '/candidate')
            self.assertIs(type(result), dict)
            self.assertEqual(set(result), channel._RESULT)
            self.assertEqual(result['exit_code'], 17)
            self.assertEqual(len(sent), 1)
            session, sent = self.session(status=17)
            owned = channel.CompletionChannel(session)
            token = self.execute(session, owned)
            self.assertEqual(owned.consume(session, token, self.argv(), '/candidate')['exit_code'], 17)
        import compiler_execution_authority as authority
        with mock.patch.object(runtime.subprocess, 'Popen', side_effect=AssertionError('no process')):
            self.assertRaises(authority.AuthorityRefusal, authority.ExecutionAuthority)
            self.assertRaises(Refusal, runtime.Session, {}, Path('/unopened'), {})
            with mock.patch.object(authority, 'preflight'):
                self.assertRaises(authority.AuthorityRefusal, authority.admit, Path('/unopened'), 'a' * 64)
