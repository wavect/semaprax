"""One-use custody for completions from the existing private command/exec.

This module issues no policy/environment authority and cannot start a process.
A completion is a runner-process object, never a mailbox/JSON proof or lease.
"""
from __future__ import annotations

import hashlib
import secrets
from dataclasses import dataclass
from types import MappingProxyType

from compiler_runtime_protocol import Refusal, encode

SCHEMA = 'semaprax.runner-command-completion.v1'
MAX_COMMANDS = 128
MAX_ARGUMENT_BYTES = 32768
MAX_STREAM_BYTES = 512
_RESULT = frozenset({'request_id', 'process_id', 'exit_code', 'stdout', 'stderr',
                     'argv', 'cwd', 'runtime_contract_sha256'})


class Completion:
    __slots__ = ()

    def __new__(cls, *_args, **_kwargs):
        raise Refusal('completion must originate on the private runtime channel')


@dataclass(frozen=True)
class _Pending:
    argv: tuple[str, ...]
    cwd: str
    request_id: int
    process_id: str
    contract: str
    binding_sha256: str
    command: dict


def _hex(value, size):
    return type(value) is str and len(value) == size and all(c in '0123456789abcdef' for c in value)


class CompletionChannel:
    def __init__(self, session):
        from compiler_shared_runtime import Session, require_authority
        if type(session) is not Session or session.initialized is not True or session.closed is not False:
            raise Refusal('live runner-owned runtime session required')
        require_authority(session.authority)
        self._owner = session
        self._authority = session.authority
        self._salt = secrets.token_bytes(32)
        self._active = None
        self._last_request = 0
        self._processes = set()

    def _owner_only(self, owner):
        from compiler_shared_runtime import require_authority
        if (owner is not self._owner or owner.authority is not self._authority
                or owner.initialized is not True or owner.closed is not False):
            raise Refusal('completion belongs to another or unavailable runtime session')
        require_authority(owner.authority)

    def _binding(self, owner, argv, cwd):
        self._owner_only(owner)
        if (type(argv) is not tuple or not 1 <= len(argv) <= 35
                or any(type(arg) is not str or '\0' in arg for arg in argv)
                or type(cwd) is not str or not cwd.startswith('/') or '\0' in cwd
                or not _hex(owner.authority.contract_sha256, 64)
                or not _hex(owner.authority.compiler_sha256, 64)
                or not _hex(owner.authority.runtime_sha256, 64)):
            raise Refusal('completion invocation binding differs')
        from compiler_shared_runtime import fixed_policy
        try:
            if (sum(len(arg.encode('utf-8')) for arg in argv) > MAX_ARGUMENT_BYTES
                    or len(cwd.encode('utf-8')) > MAX_ARGUMENT_BYTES):
                raise Refusal('completion invocation exceeds its text bound')
            encoded = encode({'argv': list(argv), 'cwd': cwd,
                'runtime_contract_sha256': owner.authority.contract_sha256,
                'runtime_sha256': owner.authority.runtime_sha256,
                'compiler_sha256': owner.authority.compiler_sha256,
                'compiler_binary': str(owner.authority.compiler_binary),
                'workspace_root': str(owner.authority.workspace_root),
                'environment': dict(owner.authority.command_env),
                'policy': fixed_policy(owner.authority)})
        except UnicodeError as error:
            raise Refusal('completion invocation is not UTF-8') from error
        return hashlib.sha256(SCHEMA.encode() + b'\0' + self._salt + encoded).hexdigest()

    def _begin(self, owner, argv, cwd, request_id, process_id):
        self._owner_only(owner)
        if (self._active is not None or type(request_id) is not int
                or request_id <= self._last_request or not _hex(process_id, 32)
                or process_id in self._processes or len(self._processes) >= MAX_COMMANDS
                or request_id != owner.sequence + 1 or request_id in owner.pending
                or type(owner.command) is not dict
                or type(owner.command.get('process_id')) is not str
                or owner.command.get('process_id') != process_id
                or 'completion' in owner.command):
            raise Refusal('command completion is nested, replayed or exceeds its bound')
        binding = self._binding(owner, argv, cwd)
        pending = _Pending(argv, cwd, request_id, process_id,
                           owner.authority.contract_sha256, binding, owner.command)
        token = object.__new__(Completion)
        self._active = (token, pending, None)
        self._last_request = request_id
        self._processes.add(process_id)
        return token

    def _complete(self, owner, token):
        # Session writes this slot only after its real request/dispatch path has
        # accepted the matching response and validated the closed result shape.
        # No caller observation dictionary crosses this completion boundary.
        self._owner_only(owner)
        if self._active is None or token is not self._active[0] or self._active[2] is not None:
            raise Refusal('command completion is absent or already completed')
        pending = self._active[1]
        try:
            result = pending.command.get('completion')
            if (owner.command is not pending.command
                    or type(pending.command.get('process_id')) is not str
                    or pending.command.get('process_id') != pending.process_id
                    or owner.sequence != pending.request_id or pending.request_id in owner.pending
                    or type(result) is not dict or set(result) != _RESULT
                    or any(type(key) is not str for key in result)
                    or type(result['request_id']) is not int or result['request_id'] != pending.request_id
                    or type(result['process_id']) is not str or result['process_id'] != pending.process_id
                    or type(result['argv']) is not tuple
                    or any(type(arg) is not str for arg in result['argv']) or result['argv'] != pending.argv
                    or type(result['cwd']) is not str or result['cwd'] != pending.cwd
                    or type(result['runtime_contract_sha256']) is not str
                    or result['runtime_contract_sha256'] != pending.contract
                    or type(result['exit_code']) is not int or not -(2**31) <= result['exit_code'] < 2**31
                    or any(type(result[key]) is not bytes or len(result[key]) > MAX_STREAM_BYTES
                           for key in ['stdout', 'stderr'])
                    or self._binding(owner, pending.argv, pending.cwd) != pending.binding_sha256):
                raise Refusal('actual runtime completion and pending command differ')
            # Every admitted value is immutable; changes to the caller dict cannot
            # alter the completion retained by the runner-owned channel.
            self._active = (token, pending, MappingProxyType(dict(result)))
            return token
        except BaseException:
            self._active = None
            raise

    def consume(self, owner, token, argv, cwd):
        self._owner_only(owner)
        active, self._active = self._active, None
        if (type(token) is not Completion or active is None or token is not active[0]
                or active[2] is None):
            raise Refusal('completion is foreign, uncompleted or already consumed')
        _, pending, result = active
        if (type(argv) is not tuple or argv != pending.argv or type(cwd) is not str
                or cwd != pending.cwd
                or self._binding(owner, argv, cwd) != pending.binding_sha256):
            raise Refusal('completion adoption invocation or runtime scope differs')
        return MappingProxyType(dict(result, completion_binding_schema=SCHEMA,
                                     completion_binding_sha256=pending.binding_sha256))

    def _abort(self, owner, token):
        # Revocation cleanup must work after authority/session drift; checking
        # current authority here could mask the original failure and skip close.
        # This releases custody only and can never start/adopt a command.
        if owner is not self._owner:
            raise Refusal('completion abort belongs to another runtime session')
        if self._active is not None and token is self._active[0]:
            self._active = None
