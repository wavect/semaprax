"""Runner-owned OS probe collection, not execution-authority admission.

No process starts at import. run_qualification is an explicit future OS action;
it is never called by campaign/Session/admit. No saved receipt mints authority.
"""
from __future__ import annotations
import base64
import ctypes
from dataclasses import dataclass
import errno
import hashlib
import json
import os
from pathlib import Path
import select
import selectors
import signal
import socket
import stat
import subprocess
import time
import uuid

from compiler_execution_authority import AuthorityRefusal, _path, _require, _within, _read
from compiler_configuration_provisioning import preflight_user_home
from compiler_config_startup_qualification import (startup_request, evaluate_startup,
    acquire_configuration, require_same_configuration)
from compiler_runtime_protocol import Protocol, Refusal, closed, decode, encode, MAX_FRAME
from compiler_shared_runtime import fixed_policy
from compiler_darwin_child import profile_source

MAX_TRANSCRIPT = 8 * 1024 * 1024
MAX_STDERR = 65536
MAX_STREAM = 16384
MAX_PROTECTION_FDS = 4096


class _DarwinAcl:
    """Exact local SDK ACL ABI; unknown/error/nonempty ACL refuses this profile."""
    def __init__(self):
        _require(os.uname().sysname == 'Darwin', 'Darwin ACL inspection required')
        self.lib = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
        self.lib.acl_get_fd_np.argtypes = [ctypes.c_int, ctypes.c_int]
        self.lib.acl_get_fd_np.restype = ctypes.c_void_p
        self.lib.acl_valid.argtypes = [ctypes.c_void_p]
        self.lib.acl_valid.restype = ctypes.c_int
        self.lib.acl_get_entry.argtypes = [ctypes.c_void_p, ctypes.c_int,
                                         ctypes.POINTER(ctypes.c_void_p)]
        self.lib.acl_get_entry.restype = ctypes.c_int
        self.lib.acl_free.argtypes = [ctypes.c_void_p]
        self.lib.acl_free.restype = ctypes.c_int

    def empty(self, fd):
        acl = self.lib.acl_get_fd_np(fd, 0x100)  # SDK ACL_TYPE_EXTENDED
        _require(bool(acl), 'ACL could not be acquired; no mode-only fallback')
        try:
            _require(self.lib.acl_valid(acl) == 0, 'invalid filesystem ACL')
            entry = ctypes.c_void_p()
            ctypes.set_errno(0)
            result = self.lib.acl_get_entry(acl, 0, ctypes.byref(entry))  # ACL_FIRST_ENTRY
            _require(result == -1 and ctypes.get_errno() == errno.EINVAL,
                     'nonempty or unsupported ACL')
        finally:
            _require(self.lib.acl_free(acl) == 0, 'ACL resource settlement failed')


def _identity(info):
    fields = ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid')
    if not stat.S_ISDIR(info.st_mode):
        fields += ('st_size', 'st_mtime_ns', 'st_ctime_ns')
    return tuple(getattr(info, key) for key in fields)


class _ProtectedStore:
    """Held root-DAC or SSD candidate observations for a NONROOT runner.

    This is one concrete supported store mechanism, not a requirement to run the
    runner as root. Same-UID chmod or an asserted read-only path is not accepted.
    In SSD candidate mode UID/mode/ACL checks establish only a stable surface,
    NOT immutability: actual kernel denial must protect against the same UID.
    It does not prove absence of privileged helpers/agent tool escapes or loader
    closure; those remain explicit qualification obligations.
    """
    def __init__(self, plan, mode):
        _require(mode in ('root-dac-empty-acl-v1', 'ssd-kernel-probe-candidate-v1'), 'unknown store collector mode')
        self.mode = mode
        _require(os.geteuid() != 0, 'probe runner must use the nonroot agent identity')
        self.acl, self.nodes = _DarwinAcl(), {}
        try:
            self.nodes[Path('/')] = (os.open('/', os.O_RDONLY | os.O_DIRECTORY), None)
            for entry in plan.inventory:
                for index in range(1, len(entry.path.parts)):
                    path = Path(*entry.path.parts[:index+1])
                    if path in self.nodes:
                        continue
                    _require(len(self.nodes) < MAX_PROTECTION_FDS, 'protected FD bound')
                    parent = self.nodes[path.parent][0]
                    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
                    if path != entry.path:
                        flags |= os.O_DIRECTORY
                    fd = os.open(path.name, flags, dir_fd=parent)
                    self.nodes[path] = (fd, None)
            for path, (fd, _) in list(self.nodes.items()):
                info = os.fstat(fd)
                allowed_owners = (0,) if mode == 'root-dac-empty-acl-v1' else (0, os.geteuid())
                _require(info.st_uid in allowed_owners and not info.st_mode & 0o022
                         and not info.st_mode & 0o6000, 'store UID/mode/setid surface refused')
                self.acl.empty(fd)
                self.nodes[path] = (fd, _identity(info))
            self.check_current()
        except BaseException:
            self.close()
            raise

    def check_current(self):
        for path, (fd, identity) in self.nodes.items():
            _require(_identity(os.fstat(fd)) == identity, 'held store identity changed')
            self.acl.empty(fd)
            if path != Path('/'):
                parent = self.nodes[path.parent][0]
                current = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
                _require(_identity(current) == identity, 'protected path rebound')

    def close(self):
        for fd, _ in reversed(list(self.nodes.values())):
            os.close(fd)
        self.nodes.clear()


class _Runtime:
    """Private qualification connection; no thread/turn/model or agent callbacks."""
    def __init__(self, plan, store, evidence, server_command, approved_commands):
        self.plan, self.store = plan, store
        self.approved_commands = frozenset(approved_commands)
        self.process_ids = set()
        self.protocol = Protocol()
        self.sequence = self.transcript_bytes = self.stderr_bytes = 0
        self.buffer = bytearray()
        self.process = None
        self.group = None
        self.selector = selectors.DefaultSelector()
        try:
            self.wire = (evidence / 'qualification-wire.jsonl').open('xb')
        except BaseException:
            self.selector.close(); raise
        try:
            self.stderr = (evidence / 'qualification-server.stderr').open('xb')
        except BaseException:
            self.wire.close(); self.selector.close(); raise
        try:
            store.check_current()
            self.process = subprocess.Popen(list(server_command), cwd=plan.server_cwd,
                env=dict(plan.server_env), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, close_fds=True, start_new_session=True)
            self.group = _acquire_group(self.process)
            os.set_blocking(self.process.stdin.fileno(), False)
            for pipe in (self.process.stdout, self.process.stderr):
                os.set_blocking(pipe.fileno(), False)
                self.selector.register(pipe, selectors.EVENT_READ)
            self.initialized = False
            params = {'clientInfo': {'name': 'semaprax-os-qualification', 'version': '1'},
                      'capabilities': {'experimentalApi': True, 'requestAttestation': False}}
            self.protocol.validate('initialize_request', {'method':'initialize','id':1,'params':params})
            self.protocol.validate('initialize_result', self.exchange('initialize', params, 20))
            self.send({'method':'initialized'})
            self.initialized = True
        except BaseException as primary:
            try: self.close()
            except BaseException as cleanup:
                try:
                    _write_new(evidence/'qualification-startup-cleanup-refusal.json',encode({
                        'type':type(cleanup).__name__,'message':str(cleanup)[:4096],
                        'selected_primary_preserved':True,'authority_granted':False}))
                except BaseException as evidence_error:
                    primary.add_note('cleanup evidence write failed: '+str(evidence_error)[:4096])
            raise

    def _record(self, direction, raw):
        self.transcript_bytes += len(raw)
        _require(self.transcript_bytes <= MAX_TRANSCRIPT, 'qualification transcript bound')
        self.wire.write(encode({'direction':direction,'raw_base64':base64.b64encode(raw).decode()}))
        self.wire.flush()

    def send(self, message):
        self.store.check_current()
        _require(type(message) is dict and message.get('method') in
                 ('initialize','initialized','config/read','command/exec'), 'qualification send scope')
        raw = encode(message)
        if message.get('method') == 'initialized':
            _require(message == {'method':'initialized'} and self.sequence == 1
                     and self.initialized is False, 'foreign initialization notification')
        else:
            _require(raw == getattr(self, '_pending_request', None),
                     'request bypassed qualification scope validation')
            self._pending_request = None
        offset = 0; deadline = time.monotonic()+5
        while offset < len(raw):
            remaining = deadline-time.monotonic()
            _require(remaining > 0, 'qualification write timeout')
            _, ready, _ = select.select([], [self.process.stdin], [], min(remaining, .2))
            if ready:
                try: offset += os.write(self.process.stdin.fileno(), raw[offset:])
                except BlockingIOError: pass
        self._record('request', raw)

    def next(self, deadline):
        while True:
            if b'\n' in self.buffer:
                raw, _, tail = self.buffer.partition(b'\n'); self.buffer = bytearray(tail)
                self._record('response', bytes(raw)+b'\n')
                return decode(bytes(raw))
            remaining = deadline-time.monotonic()
            _require(remaining > 0, 'qualification response timeout')
            for key, _ in self.selector.select(min(remaining, .2)):
                raw = os.read(key.fileobj.fileno(), 65536)
                if key.fileobj is self.process.stderr:
                    if not raw:
                        self.selector.unregister(key.fileobj); continue
                    self.stderr_bytes += len(raw)
                    _require(self.stderr_bytes <= MAX_STDERR, 'qualification stderr bound')
                    self.stderr.write(raw)
                else:
                    _require(bool(raw), 'qualification runtime EOF')
                    self.buffer.extend(raw)
                    _require(len(self.buffer) <= MAX_FRAME, 'qualification frame bound')

    def exchange(self, method, params, timeout):
        _require(method in ('initialize','config/read','command/exec'), 'qualification method refused')
        _require(type(timeout) in (int,float) and type(timeout) is not bool and 0 < timeout <= 30,
                 'qualification timeout bound')
        if method == 'initialize':
            _require(self.sequence == 0, 'duplicate initialization')
        elif method == 'config/read':
            _require(self.sequence == 1 and params == startup_request(self.plan), 'foreign config/read scope')
        else:
            self.protocol.validate(method, params)
            _require(set(params) == {'command','cwd','env','sandboxPolicy','processId','streamStdin',
                       'tty','streamStdoutStderr','outputBytesCap','timeoutMs'} and
                     tuple(params['command']) in self.approved_commands and
                     params['cwd'] == str(self.plan.workspace_root) and
                     params['env'] == dict(self.plan.command_env) and
                     params['sandboxPolicy'] == fixed_policy(self.plan) and
                     params['streamStdin'] is False and params['tty'] is False and
                     params['streamStdoutStderr'] is True and params['outputBytesCap'] == MAX_STREAM and
                     type(params['timeoutMs']) is int and 50 <= params['timeoutMs'] <= 5000,
                     'qualification command authority scope')
            process_id = params['processId']
            _require(type(process_id) is str and process_id.startswith('qual-') and
                     len(process_id) == 37 and all(c in '0123456789abcdef' for c in process_id[5:])
                     and process_id not in self.process_ids, 'qualification process identity replay')
            self.process_ids.add(process_id)
        self.sequence += 1; identifier = self.sequence
        message = {'id':identifier,'method':method,'params':params}
        self._pending_request = encode(message)
        try:
            self.send(message)
        finally:
            self._pending_request = None
        deadline = time.monotonic()+timeout
        streams = {'stdout':bytearray(), 'stderr':bytearray()}
        while True:
            message = self.next(deadline)
            _require(type(message) is dict, 'runtime frame must be an object')
            if 'id' in message:
                closed(message, ['id'], ['result','error'])
                _require(type(message['id']) is int and message['id'] == identifier and
                         (('result' in message) != ('error' in message)), 'foreign/duplicate runtime response')
                self.store.check_current()
                if 'error' in message:
                    # Preserve the whole actual error; never infer a selected exit status.
                    closed(message['error'], ['code','message'], ['data'])
                    _require(type(message['error']['code']) is int and type(message['error']['message']) is str, 'runtime error shape')
                    return {'runtime_error':message['error'], 'streams':{k:bytes(v) for k,v in streams.items()}}
                if method == 'command/exec':
                    result = message['result']; self.protocol.validate('command_result', result)
                    _require(result['stdout'] == result['stderr'] == '', 'streamed result duplicated output')
                    return {'result':result,'streams':{k:bytes(v) for k,v in streams.items()}}
                return message['result']
            self.protocol.validate('notification', message)
            if message['method'] == 'command/exec/outputDelta':
                _require(method == 'command/exec', 'output without owned command')
                row = message['params']
                _require(row['processId'] == params['processId'] and row['capReached'] is False,
                         'foreign/truncated probe output')
                raw = base64.b64decode(row['deltaBase64'], validate=True)
                streams[row['stream']].extend(raw)
                _require(len(streams[row['stream']]) <= MAX_STREAM, 'probe output bound')
            # Other schema-known notifications remain in the full raw transcript,
            # never interpreted as authority or accepted as server requests.

    def close(self):
        try:
            if self.process is not None:
                try:
                    _settle(self.process,self.group)
                    # Startup may have failed before nonblocking setup. Never
                    # enter a cleanup read on a potentially blocking pipe.
                    os.set_blocking(self.process.stdout.fileno(),False)
                    os.set_blocking(self.process.stderr.fileno(),False)
                    # Do not silently drop a late response/output after a final
                    # result. Only schema-known non-output notifications survive.
                    while True:
                        try: raw = os.read(self.process.stdout.fileno(),65536)
                        except BlockingIOError:
                            raise AuthorityRefusal('stdout held open after runtime settlement')
                        if not raw: break
                        self.buffer.extend(raw)
                        _require(len(self.buffer) <= MAX_FRAME, 'late frame bound')
                    while True:
                        try: raw = os.read(self.process.stderr.fileno(),65536)
                        except BlockingIOError:
                            raise AuthorityRefusal('stderr held open after runtime settlement')
                        if not raw: break
                        self.stderr_bytes += len(raw)
                        _require(self.stderr_bytes <= MAX_STDERR, 'late stderr bound')
                        self.stderr.write(raw)
                    self.stderr.flush()
                    while self.buffer:
                        _require(b'\n' in self.buffer, 'truncated final runtime frame')
                        raw,_,tail = self.buffer.partition(b'\n'); self.buffer = bytearray(tail)
                        self._record('response',bytes(raw)+b'\n')
                        message = decode(bytes(raw)); self.protocol.validate('notification',message)
                        _require(message['method'] != 'command/exec/outputDelta', 'late probe output')
                finally:
                    for pipe in (self.process.stdin,self.process.stdout,self.process.stderr): pipe.close()
        finally:
            self.selector.close(); self.wire.close(); self.stderr.close()


def outer_profile_source(plan, protected_roots):
    """Candidate write barrier on the SERVER and all its inherited operations.

    This does not replace inner workspace policy or claim kernel intersection.
    Paired actual direct-server/inherited-tool denials and workspace positives
    must establish its effect. External privileged deputies remain an obligation.
    """
    _require(type(protected_roots) is tuple and 0 < len(protected_roots) <= 64,
             'bounded explicit TCB roots required')
    roots = tuple(_path(str(p)) for p in protected_roots)
    _require(len(set(roots)) == len(roots), 'duplicate TCB root')
    _require(all(not _within(plan.workspace_root, r) and not _within(r, plan.workspace_root)
                 and not _within(plan.evidence_root, r) and not _within(r, plan.evidence_root)
                 for r in roots), 'TCB barrier overlaps workspace/evidence')
    config = [e.path for e in plan.inventory if e.role == 'configuration']
    _require(len(config) == 1 and all(e.path in config or any(_within(e.path,r) for r in roots)
                                     for e in plan.inventory), 'TCB root coverage incomplete')
    def quoted(path):
        text = str(path)
        _require(all(32 <= ord(c) < 127 and c not in '\\"' for c in text), 'outer profile path quoting')
        return '"' + text + '"'
    ancestors = {p for r in (*roots,*config) for p in r.parents}
    lines = ['; semaprax.app-server-tcb-write-barrier.v1', '(version 1)', '(allow default)']
    lines += ['(deny file-write* (subpath ' + quoted(r) + '))' for r in sorted(roots)]
    lines += ['(deny file-write* (literal ' + quoted(p) + '))'
              for p in sorted(ancestors | set(config))]
    raw = '\n'.join(lines)+'\n'
    _require(len(raw.encode()) <= 65536, 'outer profile bound')
    return raw



@dataclass(frozen=True)
class _CandidateConfiguration:
    contract: str
    path: Path
    data: bytes
    digest: str
    identity: tuple
    # No root-protection, held lease, execution origin or authority claim.


def _candidate_configuration(plan):
    from compiler_configuration_provisioning import validate_user_home_plan
    path = validate_user_home_plan(plan)
    entry = next(e for e in plan.inventory if e.role == 'configuration' and e.path == path)
    data, info = _read(path, entry.size)
    digest = hashlib.sha256(data).hexdigest()
    _require(len(data) == entry.size and digest == entry.sha256, 'candidate configuration drift')
    return _CandidateConfiguration(plan.contract_sha256,path,data,digest,_identity(info))


def _write_new(path, data):
    _require(type(data) is bytes, 'complete output bytes required')
    with path.open('xb') as stream:
        stream.write(data)

def _collect_plan(path, digest, mode):
    if mode == 'root-dac-empty-acl-v1':
        return preflight_user_home(path, digest)
    _require(mode == 'ssd-kernel-probe-candidate-v1', 'unknown collector mode')
    # Intentionally not preflight/authority: actual current SSD has uid501
    # ancestors. Collecting fixed policy probes is permitted without pretending
    # a root-owned leaf meets the old whole-ancestor prerequisite.
    from compiler_configuration_provisioning import parse_user_home
    raw, _ = _read(path, 262144)
    plan = parse_user_home(raw, digest)
    for entry in plan.inventory:
        data, _ = _read(entry.path, entry.size)
        _require(len(data) == entry.size and hashlib.sha256(data).hexdigest() == entry.sha256,
                 'candidate TCB artifact drift')
    return plan


@dataclass(frozen=True)
class _OwnedGroup:
    process: object
    pid: int


def _acquire_group(process):
    # Popen is owned here, single-threaded, and the direct child is unreaped.
    # An unreaped child PID cannot be reassigned to an unrelated process.
    _require(process.returncode is None and process.pid > 1
             and process.pid != os.getpgrp(), 'no unreaped owned group leader')
    _require(os.getpgid(process.pid) == process.pid, 'child is not its own group leader')
    return _OwnedGroup(process,process.pid)


def _observe_group_absent(pid):
    # Observation only after reaping: never signal this numeric PG again.
    deadline = time.monotonic()+2
    while True:
        try: os.killpg(pid,0)
        except ProcessLookupError: return
        _require(time.monotonic() < deadline,
                 'residual or reused process group observed; no authority to signal it')
        time.sleep(.02)


def _settle(process, owner=None):
    _require(process.pid > 1 and process.pid != os.getpgrp(), 'foreign process group')
    if process.returncode is not None:
        _observe_group_absent(process.pid)
        return
    if owner is None:
        owner = _acquire_group(process)
    _require(owner.process is process and owner.pid == process.pid, 'foreign group ownership')
    for sig in (signal.SIGTERM, signal.SIGKILL):
        # Do not poll/wait before this check: those calls can reap the leader.
        _require(process.returncode is None and os.getpgid(owner.pid) == owner.pid,
                 'unreaped owned leader continuity lost')
        try: os.killpg(owner.pid,sig)
        except ProcessLookupError:
            process.wait(timeout=2)
            _observe_group_absent(owner.pid)
            return
        try: process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            _require(process.returncode is None, 'unexpected reaped child after timeout')
            continue
        _observe_group_absent(owner.pid)
        return
    raise AuthorityRefusal('owned producer leader did not settle after bounded signals')


def _build_probe(plan, store, compiler, evidence):
    """Actual fixed-source producer, not a static manifest claiming build origin.

    Its cost is fixed harness/qualification cost, never model tokens. The
    provisioned native build compiler/SDK/loader remain part of the declared TCB.
    """
    compiler = _path(str(compiler))
    _require(sum(e.path == compiler and e.role == 'dependency' for e in plan.inventory) == 1,
             'declared probe build compiler required')
    source = Path(__file__).with_name('compiler_runtime_v1')/'darwin_probe.c'
    data, _ = _read(source, 32768)
    _require(hashlib.sha256(data).hexdigest() == PROBE_SOURCE_SHA256, 'fixed native probe source drift')
    source_path, binary = evidence/'fixed-probe.c', evidence/'fixed-probe'
    _write_new(source_path,data)
    args = [str(compiler), '-std=c11', '-D_DARWIN_C_SOURCE=1', '-Wall', '-Wextra', '-Werror',
            '-O2', str(source_path), '-o', str(binary)]
    env = dict(plan.server_env, TMPDIR=str(evidence), HOME=str(evidence),
               CODEX_HOME=str(evidence/'.codex'), CLANG_MODULE_CACHE_PATH=str(evidence/'clang-modules'))
    store.check_current()
    process = subprocess.Popen(args, cwd=evidence, env=env, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               close_fds=True, start_new_session=True)
    selector = None; group = None
    outputs = {'stdout':bytearray(),'stderr':bytearray()}
    started = time.monotonic(); code = None; primary = None; cleanup_error = None
    try:
        group = _acquire_group(process)
        selector = selectors.DefaultSelector()
        for name in outputs:
            pipe = getattr(process,name); os.set_blocking(pipe.fileno(),False)
            selector.register(pipe, selectors.EVENT_READ, name)
        while selector.get_map():
            _require(time.monotonic()-started < 30, 'fixed probe build timeout')
            for key,_ in selector.select(.1):
                block = os.read(key.fileobj.fileno(),65536)
                if not block: selector.unregister(key.fileobj); continue
                outputs[key.data].extend(block)
                _require(len(outputs[key.data]) <= MAX_STDERR, 'fixed probe build output bound')
        code = process.wait(timeout=2)
    except BaseException as error:
        primary = error
    finally:
        try: _settle(process,group)
        except BaseException as error: cleanup_error = error
        finally:
            if selector is not None: selector.close()
            process.stdout.close(); process.stderr.close()
    record = {'schema':'semaprax.actual-fixed-probe-build.v1','command':args,'env':env,
              'source_sha256':PROBE_SOURCE_SHA256,'exit_code':code,
              'elapsed_seconds':time.monotonic()-started,
              'observed_terminal_after_guard':process.returncode,
              'selected_error':None if primary is None else {'type':type(primary).__name__,'message':str(primary)},
              'cleanup_error':None if cleanup_error is None else {'type':type(cleanup_error).__name__,'message':str(cleanup_error)},
              'stdout_base64':base64.b64encode(outputs['stdout']).decode(),
              'stderr_base64':base64.b64encode(outputs['stderr']).decode()}
    if primary is not None or cleanup_error is not None or code != 0:
        _write_new(evidence/'actual-probe-build.json',encode(record))
        if primary is not None: raise primary
        if cleanup_error is not None: raise cleanup_error
        raise AuthorityRefusal('actual fixed probe build failed')
    try:
        image, info = _read(binary, 4*1024*1024)
    except BaseException as error:
        record['artifact_error'] = {'type':type(error).__name__,'message':str(error)}
        _write_new(evidence/'actual-probe-build.json',encode(record))
        raise
    _require(info.st_mode & 0o111 and not info.st_mode & 0o6000, 'probe executable mode')
    record['binary_sha256'] = hashlib.sha256(image).hexdigest()
    _write_new(evidence/'actual-probe-build.json',encode(record))
    store.check_current()
    return binary, record['binary_sha256']


def _probe_document(raw, case):
    value = decode(raw)
    _require(type(value) is dict and value.get('schema') == 'semaprax.darwin-probe.v1'
             and value.get('case') == case and type(value.get('pid')) is int and value['pid'] > 1
             and type(value.get('uid')) is int and value['uid'] == os.geteuid(), 'foreign probe observation')
    keys = ['schema','case','pid','uid','entries_hex'] if case == 'environment' else ['schema','case','pid','uid','result','errno']
    closed(value,keys)
    if case == 'environment':
        _require(type(value['entries_hex']) is list and len(value['entries_hex']) <= 64
                 and all(type(x) is str and len(x) <= 32768 for x in value['entries_hex']), 'probe environment shape')
    else:
        _require(type(value['result']) is int and type(value['errno']) is int
                 and value['errno'] >= 0, 'probe operation shape')
    return value


def _check_probe(case, outcome, env, *, denied=False):
    _require('result' in outcome and not outcome['streams']['stderr'], 'probe runtime failure/error output')
    _require(outcome['result']['exitCode'] == 0, 'native probe failed')
    observed = _probe_document(outcome['streams']['stdout'],case)
    if case == 'environment':
        expected = sorted((k+'='+v).encode().hex() for k,v in env.items())
        _require(sorted(observed['entries_hex']) == expected, 'server-computed environment not closed')
    elif denied:
        _require(observed['result'] == -1 and observed['errno'] in (errno.EACCES,errno.EPERM),
                 'operation was not denied by permission policy')
    else:
        _require(observed['result'] == 0 and observed['errno'] == 0, 'ordinary positive control failed')
    return observed


def _probe_params(plan, command, timeout_ms=1000):
    return {'command':list(command),'cwd':str(plan.workspace_root),
            'env':dict(plan.command_env),'sandboxPolicy':fixed_policy(plan),
            'processId':'qual-'+uuid.uuid4().hex, 'streamStdin':False, 'tty':False,
            'streamStdoutStderr':True,'outputBytesCap':MAX_STREAM,'timeoutMs':timeout_ms}



def _termination_observation(case, outcome):
    # Signal/timeout results are preserved whole. We never guess an exit mapping
    # or infer a missing status from output. These are NOT whole-child proofs.
    _require(case in ('signal','timeout'), 'unknown termination probe')
    observed = _probe_document(outcome['streams']['stdout'],case)
    if 'result' in outcome:
        _require(outcome['result']['exitCode'] != 0, 'termination control returned success')
    else:
        _require('runtime_error' in outcome, 'missing termination status/error')
    deadline = time.monotonic()+3
    while True:
        try: os.kill(observed['pid'],0)
        except ProcessLookupError: break
        _require(time.monotonic() < deadline, 'direct probe PID still observed alive')
        time.sleep(.02)
    return {'actual_probe':observed,
            'actual_terminal':outcome.get('result'),
            'actual_runtime_error':outcome.get('runtime_error'),
            'stderr_base64':base64.b64encode(outcome['streams']['stderr']).decode(),
            'scope':'DIRECT_PID_OBSERVATION_NOT_DESCENDANT_SETTLEMENT'}

def _make_private_directory(path):
    """Create a fresh leaf through no-follow parent handles, without a lease claim.

    Later path use still needs the qualified outer barrier and runner-private
    lifetime; these creation checks alone cannot close same-UID rebinding.
    """
    path = _path(str(path))
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in path.parent.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=fd)
            os.close(fd); fd = child
        os.mkdir(path.name, mode=0o700, dir_fd=fd)
        child = os.open(path.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                        dir_fd=fd)
        try:
            info = os.fstat(child)
            _require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700,
                     'private directory owner/mode refused')
            _require(_identity(info) == _identity(os.stat(path,follow_symlinks=False)),
                     'private directory rebound during acquisition')
        finally: os.close(child)
    finally: os.close(fd)


def _record_primary_refusal(evidence,error,results):
    try:
        _write_new(evidence/'qualification-refusal.json',encode({
            'status':'ACTUAL_ATTEMPT_REFUSED_NO_AUTHORITY','type':type(error).__name__,
            'message':str(error)[:4096],'completed_observations':results,'authority_granted':False}))
    except BaseException as evidence_error:
        error.add_note('primary evidence write failed: '+str(evidence_error)[:4096])


def _cleanup_runtime(runtime,evidence,primary,results):
    try: runtime.close()
    except BaseException as error:
        try:
            _write_new(evidence/'qualification-cleanup-refusal.json',encode({
                'type':type(error).__name__,'message':str(error)[:4096],
                'selected_primary_preserved':primary is not None,
                'completed_observations':results,'authority_granted':False}))
        except BaseException as evidence_error:
            if primary is None: raise error from evidence_error
            primary.add_note('cleanup evidence write failed: '+str(evidence_error)[:4096])
            return  # The already selected primary is reraised by the caller.
        if primary is None: raise


def run_qualification(contract_path, contract_sha256, *, evidence, store_mode,
                      protected_roots, sandbox_launcher, probe_build_compiler, model, effort):
    """Explicit future probe action. No agent/model turn and no authority grant.

    A source-only invocation is NOT performed by import or any current campaign.
    Parent must separately authorize/schedule actual OS producer/probe execution.
    """
    _require(os.uname().sysname == 'Darwin', 'Darwin qualification required')
    plan = _collect_plan(Path(contract_path), contract_sha256, store_mode)
    evidence = _path(str(evidence))
    _require(_within(evidence,plan.evidence_root) and evidence != plan.evidence_root
             and not any(_within(evidence,r) or _within(r,evidence) for r in plan.agent_writable_roots),
             'exclusive evidence outside agent roots required')
    launcher = _path(str(sandbox_launcher))
    _require(sum(e.path == launcher and e.role == 'dependency' for e in plan.inventory) == 1,
             'declared sandbox launcher required')
    outer = outer_profile_source(plan,protected_roots)
    _make_private_directory(evidence)
    store = _ProtectedStore(plan,store_mode)
    results = []
    try:
        binary, image_sha = _build_probe(plan,store,Path(probe_build_compiler),evidence)
        marker = evidence/'private-denial-marker'; _write_new(marker,b'K')
        publication = plan.workspace_root/('qualification-'+uuid.uuid4().hex)
        _make_private_directory(publication)
        positive = publication/'marker'
        _write_new(positive,b'K')
        server_command = (str(launcher),'-p',outer,*plan.server_argv)
        reads = tuple(sorted({e.path for e in plan.inventory} | {binary,positive,Path('/dev/fd'),Path('/dev/null')}))
        profile = profile_source(binary,reads,publication)
        cases = [('environment',None,False),('descriptors',None,False),('read',positive,False),
                 ('write',positive,False),('read',marker,True),('write',marker,True),
                 ('fork',None,True),('foreign-exec',Path('/usr/bin/true'),True),('network',None,True)]
        unix_path = evidence/'s'
        _require(len(os.fsencode(unix_path)) < 104, 'Darwin Unix socket path bound')
        unix = socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
        try:
            unix.bind(str(unix_path)); unix.listen(1)
        except BaseException:
            unix.close(); raise
        cases.append(('unix-socket',unix_path,True))
        commands = [(str(launcher),'-p',profile,str(binary),case)+((str(target),) if target else ())
                    for case,target,_ in cases]
        terminal_commands = [(str(launcher),'-p',profile,str(binary),case)
                             for case in ('signal','timeout')]
        runtime = None
        primary = None
        try:
            runtime = _Runtime(plan,store,evidence,server_command,commands+terminal_commands)
            if store_mode == 'ssd-kernel-probe-candidate-v1':
                acquire = _candidate_configuration
            else:
                acquire = acquire_configuration
            before = acquire(plan)
            params = startup_request(plan)
            result = runtime.exchange('config/read',params,20)
            after = acquire(plan)
            if store_mode == 'ssd-kernel-probe-candidate-v1':
                _require(before == after, 'candidate startup configuration endpoint drift')
            else:
                require_same_configuration(before,after)
            startup = evaluate_startup(plan,request=params,response=result,
                held_config_bytes=before.data,model=model,effort=effort)
            results.append({'case':'actual-config-startup','result_sha256':startup.response_result_sha256})
            # Probe profile is explicitly instrumented, not mislabeled as the
            # exact compiler profile: its exec target is fixed native probe and
            # /dev/fd is readable solely for descriptor observation.
            for (case,target,denied),command in zip(cases,commands):
                current,_ = _read(binary,4*1024*1024)
                _require(hashlib.sha256(current).hexdigest() == image_sha,'actual built probe drift')
                outcome = runtime.exchange('command/exec',_probe_params(plan,command),5)
                observed = _check_probe(case,outcome,plan.command_env,denied=denied)
                results.append({'case':case,'denied':denied,'actual':observed,
                    'profile_scope':'INSTRUMENTED_NATIVE_PROBE_NOT_COMPILER_PROFILE',
                    'profile_sha256':hashlib.sha256(profile.encode()).hexdigest()})
                _require(marker.read_bytes() == b'K','denied evidence was mutated')
            for case,command in zip(('signal','timeout'),terminal_commands):
                outcome = runtime.exchange('command/exec',_probe_params(plan,command,100),5)
                results.append({'case':case,'actual':_termination_observation(case,outcome)})
        except BaseException as error:
            primary = error
            _record_primary_refusal(evidence,error,results)
            raise
        finally:
            try:
                if runtime is not None:
                    _cleanup_runtime(runtime,evidence,primary,results)
            finally: unix.close()
        record = {'schema':'semaprax.darwin-qualification-observations.v1',
            'status':'ACTUAL_CONTROL_OBSERVATIONS_NOT_AUTHORITY', 'store_mode':store_mode,
            'contract_sha256':plan.contract_sha256,'runtime_sha256':plan.runtime_sha256,
            'outer_profile_sha256':hashlib.sha256(outer.encode()).hexdigest(),
            'results':results,'authority_granted':False,
            'unresolved':['actual direct app-server write and ordinary positive control',
                'client-loss/timeout semantic attribution and whole-child settlement beyond direct PID',
                'privileged-deputy/whole-agent-tool escape coverage',
                'actual complete loader/interpreter closure and exact compiler route',
                'live authority lifetime/grant implementation and matched calibration']}
        _write_new(evidence/'qualification-observations.json',encode(record))
        return record
    finally:
        store.close()

PROBE_SOURCE_SHA256 = "df57cb4e9802d4b87762f55a3387c2eafbc9a5fd7aee9734e1666f2a58b920d2"
