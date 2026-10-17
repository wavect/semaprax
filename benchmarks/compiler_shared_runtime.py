"""Opt-in shared-runtime transport v1; never a replacement for codex-exec v1.

Only an admitted enforcement authority may launch this runner-private connection.
No nonce, profile name, schema, static hash or caller dictionary grants admission.
"""
from __future__ import annotations
import base64
import hashlib
import os
from pathlib import Path
import selectors
import select
import signal
import subprocess
import time
import uuid

from compiler_runtime_protocol import Protocol, Refusal, closed, decode, encode, MAX_FRAME

BACKEND = 'semaprax.shared-runtime-compiler.v1'
TOOL = 'semaprax_retained_codec'
MAX_MESSAGES = 20000
MAX_TRANSCRIPT = 32 * 1024 * 1024
MAX_STDERR = 1024 * 1024


def matched_configuration(value):
    closed(value, ['backend', 'model', 'effort', 'arms', 'runtime_contract_sha256',
                   'compiler_tool_arms', 'allow_model_fallback'])
    if (value['backend'] != BACKEND or value['arms'] != ['semaprax', 'typescript']
            or value['compiler_tool_arms'] != ['semaprax']
            or value['allow_model_fallback'] is not False
            or type(value['model']) is not str or not 1 <= len(value['model']) <= 128
            or value['effort'] not in ['low', 'medium', 'high', 'xhigh', 'max', 'ultra']
            or type(value['runtime_contract_sha256']) is not str
            or len(value['runtime_contract_sha256']) != 64
            or any(c not in '0123456789abcdef' for c in value['runtime_contract_sha256'])):
        raise Refusal('shared-runtime matched configuration differs')
    # Freeze through the same closed JSON representation used for the receipt.
    return decode(encode(value))


def require_authority(authority):
    try:
        from compiler_execution_authority import ExecutionAuthority
    except ImportError as error:
        raise Refusal('qualified runtime enforcement implementation unavailable') from error
    if type(authority) is not ExecutionAuthority:
        raise Refusal('qualified runtime enforcement authority unavailable')
    authority.check_current()


class Session:
    def __init__(self, authority, evidence: Path, configuration):
        require_authority(authority)
        self.authority = authority
        self.config = matched_configuration(configuration)
        if self.config['runtime_contract_sha256'] != authority.contract_sha256:
            raise Refusal('configuration and authority contract differ')
        authority.check_private_path(evidence)
        self.protocol = Protocol()
        self.sequence = 0
        self.pending = set()
        self.responses = {}
        self.events = []
        self.command = None
        self.handler = None
        self.deferred_requests = []
        self.server_ids = set()
        self.call_ids = set()
        self.messages = 0
        self.total = 0
        self.stderr_bytes = 0
        self.buffer = bytearray()
        self.thread_id = None
        self.turn_id = None
        self.closed = False
        self.initialized = False
        self.process = None
        evidence.mkdir()  # exclusive; no existing evidence reused
        self.transcript = (evidence / 'runtime.jsonl').open('xb')
        self.stderr = (evidence / 'runtime.stderr').open('xb')
        self.selector = selectors.DefaultSelector()
        try:
            authority.check_current()
            self.process = subprocess.Popen(list(authority.server_argv),
                cwd=authority.server_cwd, env=dict(authority.server_env),
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                close_fds=True, start_new_session=True)
            os.set_blocking(self.process.stdin.fileno(), False)
            for pipe in [self.process.stdout, self.process.stderr]:
                os.set_blocking(pipe.fileno(), False)
                self.selector.register(pipe, selectors.EVENT_READ)
            request = {'method': 'initialize', 'id': 1, 'params': {
                'clientInfo': {'name': 'semaprax-matched-runtime', 'version': '1'},
                'capabilities': {'experimentalApi': True, 'requestAttestation': False}}}
            self.protocol.validate('initialize_request', request)
            self.protocol.validate('initialize_result', self.request('initialize', request['params'], 20))
            self.send({'method': 'initialized'})
            self.initialized = True
        except BaseException:
            self.close()
            raise

    def send(self, message):
        if self.closed or self.process is None or self.process.poll() is not None:
            raise Refusal('runtime connection is not live')
        raw = encode(message)
        deadline = time.monotonic() + 10
        position = 0
        while position < len(raw):
            if time.monotonic() >= deadline:
                raise Refusal('runtime request write timeout')
            _, ready, _ = select.select([], [self.process.stdin], [], min(0.2, deadline - time.monotonic()))
            if ready:
                try:
                    position += os.write(self.process.stdin.fileno(), raw[position:])
                except BlockingIOError:
                    pass

    def _record(self, raw):
        self.total += len(raw)
        self.messages += 1
        if self.total > MAX_TRANSCRIPT or self.messages > MAX_MESSAGES:
            raise Refusal('runtime transcript exceeds bound')
        self.transcript.write(raw)
        self.transcript.flush()

    def _next(self, deadline):
        while True:
            newline = self.buffer.find(b'\n')
            if newline >= 0:
                raw = bytes(self.buffer[:newline + 1])
                del self.buffer[:newline + 1]
                self._record(raw)
                return decode(raw)
            if len(self.buffer) > MAX_FRAME:
                raise Refusal('runtime frame exceeds bound')
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise Refusal('runtime response timeout')
            ready = self.selector.select(min(remaining, 0.2))
            for key, _ in ready:
                raw = os.read(key.fd, 65536)
                if not raw:
                    self.selector.unregister(key.fileobj)
                    if key.fileobj is self.process.stdout:
                        raise Refusal('runtime EOF before complete response')
                elif key.fileobj is self.process.stdout:
                    self.buffer.extend(raw)
                else:
                    self.stderr_bytes += len(raw)
                    if self.stderr_bytes > MAX_STDERR:
                        raise Refusal('runtime stderr exceeds bound')
                    self.stderr.write(raw)
                    self.stderr.flush()

    def _dispatch(self, message):
        if type(message) is not dict:
            raise Refusal('runtime message is not an object')
        if 'method' in message:
            if 'id' in message:
                self.protocol.validate('server_request', message)
                identifier = message['id']
                if type(identifier) not in (int, str) or identifier in self.server_ids:
                    raise Refusal('duplicate runtime server request')
                self.server_ids.add(identifier)
                if message['method'] != 'item/tool/call' or self.handler is None:
                    # Never approve permission, shell, file, network or auth requests.
                    raise Refusal('runtime requested unavailable authority')
                params = message['params']
                if self.turn_id is None:
                    if len(self.deferred_requests) >= 8:
                        raise Refusal('early runtime tool queue exceeds bound')
                    self.deferred_requests.append(message)
                    return
                if params['callId'] in self.call_ids:
                    raise Refusal('replayed runtime compiler call')
                self.call_ids.add(params['callId'])
                if (params['threadId'] != self.thread_id or params['turnId'] != self.turn_id
                        or params['tool'] != TOOL or params.get('namespace') is not None):
                    raise Refusal('foreign compiler tool call')
                result = self.handler(params['arguments'])
                self.protocol.validate('tool_result', result)
                self.send({'id': message['id'], 'result': result})
            else:
                self.protocol.validate('notification', message)
                if message['method'] == 'command/exec/outputDelta':
                    self._command_delta(message['params'])
                else:
                    # Kept verbatim for the versioned campaign trace, never as
                    # compiler origin/status or policy/environment authority.
                    if len(self.events) >= 20000:
                        raise Refusal('runtime event queue exceeds bound')
                    self.events.append(message)
            return
        closed(message, ['id'], ['result', 'error'])
        if (type(message['id']) is not int or message['id'] not in self.pending
                or message['id'] in self.responses
                or ('result' in message) == ('error' in message)):
            raise Refusal('foreign, duplicate or malformed runtime response')
        self.responses[message['id']] = message

    def request(self, method, params, timeout):
        if not 0 < timeout <= 1800 or len(self.pending) >= 8:
            raise Refusal('runtime request bound differs')
        if method != 'initialize':
            self.protocol.validate(method, params)
        self.sequence += 1
        identifier = self.sequence
        self.pending.add(identifier)
        try:
            self.send({'method': method, 'id': identifier, 'params': params})
            deadline = time.monotonic() + timeout
            while identifier not in self.responses:
                self._dispatch(self._next(deadline))
            reply = self.responses.pop(identifier)
            if 'error' in reply:
                raise Refusal('runtime refused request; no fallback execution')
            return reply['result']
        finally:
            self.pending.remove(identifier)

    def start_thread(self, arm):
        require_authority(self.authority)
        if arm not in self.config['arms'] or self.thread_id is not None:
            raise Refusal('shared-runtime arm/thread differs')
        tools = []
        if arm in self.config['compiler_tool_arms']:
            tools = [{'type': 'function', 'name': TOOL,
                'description': 'Run one retained-receipt codec through the trusted runner sandbox.',
                'inputSchema': {'type': 'object', 'properties': {
                    'cwd': {'type': 'string'}, 'argv': {'type': 'array', 'items': {'type': 'string'}}},
                    'required': ['cwd', 'argv'], 'additionalProperties': False}}]
        params = {'cwd': str(self.authority.workspace_root), 'model': self.config['model'],
            'sandbox': 'workspace-write', 'approvalPolicy': 'never',
            'runtimeWorkspaceRoots': [str(self.authority.workspace_root)],
            'environments': [], 'allowProviderModelFallback': False, 'dynamicTools': tools,
            'config': {'model_reasoning_effort': self.config['effort']}}
        result = self.protocol.validate('thread_start_result', self.request('thread/start', params, 20))
        if (result['cwd'] != str(self.authority.workspace_root)
                or result['runtimeWorkspaceRoots'] != [str(self.authority.workspace_root)]
                or result['sandbox'] != decode(encode(dict(self.authority.sandbox_policy)))
                or result['approvalPolicy'] != 'never'
                or result['model'] != self.config['model']
                or result['reasoningEffort'] != self.config['effort']):
            raise Refusal('runtime effective thread binding differs')
        self.thread_id = result['thread']['id']
        return result

    def run_turn(self, prompt, handler, timeout=1800):
        require_authority(self.authority)
        if self.thread_id is None or self.turn_id is not None:
            raise Refusal('runtime turn is not fresh')
        self.handler = handler
        params = {'threadId': self.thread_id, 'input': [{'type': 'text', 'text': prompt}],
            'cwd': str(self.authority.workspace_root),
            'runtimeWorkspaceRoots': [str(self.authority.workspace_root)],
            'environments': [], 'approvalPolicy': 'never',
            'sandboxPolicy': decode(encode(dict(self.authority.sandbox_policy))),
            'model': self.config['model'], 'effort': self.config['effort']}
        result = self.protocol.validate('turn_start_result', self.request('turn/start', params, 20))
        self.turn_id = result['turn']['id']
        deferred, self.deferred_requests = self.deferred_requests, []
        for message in deferred:
            self.server_ids.remove(message['id'])
            self._dispatch(message)
        deadline = time.monotonic() + timeout
        while True:
            for index, event in enumerate(self.events):
                if event['method'] == 'turn/completed':
                    params = event['params']
                    if params['threadId'] != self.thread_id or params['turn']['id'] != self.turn_id:
                        raise Refusal('foreign runtime turn completion')
                    self.events.pop(index)
                    return {'backend': BACKEND, 'arm_configuration': self.config,
                            'thread_id': self.thread_id, 'turn': params['turn'],
                            'actual_billed_usd': None, 'ratio_eligible': False}
            self._dispatch(self._next(deadline))

    def _command_delta(self, value):
        # Shape was already checked against the exact installed notification schema.
        if self.command is None or value['processId'] != self.command['process_id']:
            raise Refusal('foreign command output')
        if value['capReached'] is not False:
            raise Refusal('compiler output was truncated')
        stream = value['stream']
        if stream not in ['stdout', 'stderr']:
            raise Refusal('unknown compiler output stream')
        try:
            raw = base64.b64decode(value['deltaBase64'], validate=True)
        except (ValueError, TypeError) as error:
            raise Refusal('malformed compiler output encoding') from error
        self.command[stream].extend(raw)
        if len(self.command[stream]) > 512:
            raise Refusal('compiler output exceeds trusted observation bound')

    def execute(self, argv, cwd, timeout=60):
        require_authority(self.authority)
        self.authority.authorize_cwd(cwd)
        if self.command is not None or not 0 < timeout <= 60:
            raise Refusal('nested or unbounded compiler execution')
        argv = tuple(argv)
        if (not argv or argv[0] != str(self.authority.compiler_binary)
                or any(type(arg) is not str or '\0' in arg for arg in argv)):
            raise Refusal('compiler executable or argv differs')
        process_id = uuid.uuid4().hex
        self.command = {'process_id': process_id, 'stdout': bytearray(), 'stderr': bytearray()}
        params = {'command': list(argv), 'cwd': cwd, 'env': dict(self.authority.command_env),
            'sandboxPolicy': decode(encode(dict(self.authority.sandbox_policy))),
            'processId': process_id, 'tty': False, 'streamStdin': False,
            'streamStdoutStderr': True, 'outputBytesCap': 513, 'timeoutMs': int(timeout * 1000)}
        try:
            request_id = self.sequence + 1
            result = self.protocol.validate('command_result', self.request('command/exec', params, timeout + 5))
            if result['stdout'] != '' or result['stderr'] != '' or type(result['exitCode']) is not int:
                raise Refusal('streamed runtime completion differs')
            return {'request_id': request_id, 'process_id': process_id, 'exit_code': result['exitCode'],
                'stdout': bytes(self.command['stdout']), 'stderr': bytes(self.command['stderr']),
                'argv': tuple(argv), 'cwd': cwd,
                'runtime_contract_sha256': self.authority.contract_sha256}
        except BaseException:
            # A refused observation cannot become a success by accepting proxy
            # completion or retrying. Whole private runtime is terminated below.
            self.close()
            raise
        finally:
            self.command = None

    def close(self):
        if self.closed:
            return
        self.closed = True
        process = self.process
        settlement_error = None
        if process is not None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=2)
            # Prove settlement of this runner-owned server group. A qualified
            # authority must separately establish the runtime command lifecycle;
            # this check never claims to enumerate detached host processes.
            deadline = time.monotonic() + 2
            while True:
                try:
                    os.killpg(process.pid, 0)
                except ProcessLookupError:
                    break
                if time.monotonic() >= deadline:
                    settlement_error = Refusal('runner-owned runtime group did not settle')
                    break
                os.killpg(process.pid, signal.SIGKILL)
                time.sleep(0.01)
            for pipe in [process.stdin, process.stdout, process.stderr]:
                pipe.close()
        self.selector.close()
        self.transcript.close()
        self.stderr.close()
        if settlement_error is not None:
            raise settlement_error

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
