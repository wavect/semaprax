"""Finite compiler child transition, intended ONLY below trusted command/exec.

This module mints no ExecutionAuthority or trusted execution receipt. Installing
its source/profile is not qualification. Session's live grant gate remains closed.
The direct exec transition preserves the runtime-owned PID; no local replay or
shell is used. Kernel no-fork and complete runtime settlement require qualification.
"""
from __future__ import annotations

from dataclasses import dataclass, field
import errno
import hashlib
import os
from pathlib import Path
import stat
from types import MappingProxyType

from compiler_execution_authority import (AuthorityRefusal, ProvisioningPlan,
                                           _path, _read, _require, _within)
import compiler_retained_receipt as retained

PROFILE_VERSION = "semaprax.darwin-compiler-child.v1"
MAX_FILES = 1024
MAX_ARGUMENT_BYTES = 16384
MAX_PROFILE_BYTES = 262144
MAX_FDS = 4096


@dataclass(frozen=True)
class ChildTransition:
    """Unqualified launch recipe, never an authority grant or success receipt."""
    compiler: Path
    sandbox_launcher: Path
    inventory: tuple
    cwd: Path
    argv: tuple[str, ...]
    environment: object
    profile: str
    profile_sha256: str
    publication_directory: Path
    read_files: tuple[Path, ...]
    contract_sha256: str


def _literal(path):
    text = str(_path(str(path)))
    # Seatbelt string syntax is deliberately narrower than arbitrary POSIX names.
    _require(all(32 <= ord(c) < 127 and c not in '\\"' for c in text),
             "unsupported sandbox path quoting")
    return '(literal "' + text + '")'


def profile_source(compiler, read_files, publication_directory):
    """Generate one deny-default candidate. No inherited/imported profile rules."""
    compiler = _path(str(compiler))
    publication_directory = _path(str(publication_directory))
    _require(type(read_files) is tuple and 0 < len(read_files) <= MAX_FILES,
             "bounded read inventory required")
    paths = tuple(_path(str(p)) for p in read_files)
    _require(len(set(paths)) == len(paths) and compiler in paths,
             "unique inventory including compiler required")
    metadata = {parent for p in paths + (publication_directory,)
                for parent in p.parents if parent != Path('/')}
    # No process-fork, networking, socket, mach-lookup or arbitrary exec grant.
    # Exact operation semantics/startup dependencies must pass Darwin probes.
    lines = ['; ' + PROFILE_VERSION, '(version 1)', '(deny default)',
             '(allow process-exec ' + _literal(compiler) + ')']
    lines += ['(allow file-read* ' + _literal(p) + ')' for p in sorted(paths)]
    lines += ['(allow file-read-metadata ' + _literal(p) + ')'
              for p in sorted(metadata)]
    # Compiler publication uses a new runner-owned directory, including its
    # atomic temporary file. It never receives write authority to project root.
    quoted = _literal(publication_directory).removeprefix('(literal ').removesuffix(')')
    lines += ['(allow file-read* file-write* (subpath ' + quoted + '))']
    text = '\n'.join(lines) + '\n'
    _require(len(text.encode()) <= MAX_PROFILE_BYTES, "sandbox profile exceeds bound")
    return text


def prepare_transition(plan, argv, input_files, publication_directory, sandbox_launcher):
    """Resolve exact argv/environment and candidate policy; perform no launch.

    plan must already come from provisioning preflight, but remains static data.
    The trusted caller must retain captured inputs and later qualify enforcement.
    """
    _require(type(plan) is ProvisioningPlan, "provisioning plan required")
    _require(type(argv) is tuple and 1 <= len(argv) <= 32 and
             all(type(a) is str and '\0' not in a for a in argv) and
             sum(len(os.fsencode(a)) for a in argv) <= MAX_ARGUMENT_BYTES,
             "closed bounded compiler argv required")
    _require(len(argv) >= 2 and argv[-2] == '--receipt-stdout-seal' and
             len(argv[-1]) == 64 and all(c in '0123456789abcdef' for c in argv[-1]),
             "runner sealed invocation required")
    values = retained.options(list(argv[:-2]))
    _require(values is not None and values['project'] == '.',
             "fixed-root retained compiler grammar required")
    _require(type(input_files) is tuple and 0 < len(input_files) <= MAX_FILES,
             "bounded captured input files required")
    inputs = tuple(_path(str(p)) for p in input_files)
    _require(len(set(inputs)) == len(inputs) and
             all(_within(p, plan.workspace_root) for p in inputs),
             "input inventory escapes trial root")
    publication = _path(str(publication_directory))
    _require(publication.parent == plan.workspace_root and
             _within(publication, plan.workspace_root) and
             not any(_within(p, publication) for p in inputs),
             "exclusive publication directory overlaps input/root")
    source = Path(values['--source'])
    _require(not source.is_absolute() and source.parts and
             all(p not in ('..', '.') for p in source.parts) and
             plan.workspace_root / source in inputs and
             plan.workspace_root / 'semaprax.toml' in inputs,
             "schema and manifest must be in captured input inventory")
    output = Path(values['--output'])
    _require(not output.is_absolute() and all(p not in ('..', '.') for p in output.parts),
             "relative output required")
    output = plan.workspace_root / output
    _require(output.parent == publication, "output must use dedicated publication directory")
    # Actual exec environment is rebuilt, not merged with runtime overrides.
    env = dict(plan.command_env)
    _require(set(env) == {'PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'TZ'} and
             env == dict(plan.server_env) and
             all(type(v) is str and '\0' not in v for v in env.values()),
             "closed actual child environment required")
    _require(env['LANG'] == 'C' and env['LC_ALL'] == 'C' and env['TZ'] == 'UTC',
             "fixed child locale required")
    sandbox_launcher = _path(str(sandbox_launcher))
    _require(sum(e.path == sandbox_launcher and e.role == 'dependency'
                 for e in plan.inventory) == 1, "pinned sandbox launcher dependency required")
    _require(not _within(sandbox_launcher, plan.workspace_root),
             "sandbox launcher overlaps writable trial")
    protected = tuple(e.path for e in plan.inventory)
    reads = tuple(sorted(set(protected + inputs)))
    profile = profile_source(plan.compiler_binary, reads, publication)
    return ChildTransition(plan.compiler_binary, sandbox_launcher, plan.inventory, plan.workspace_root,
                           (str(plan.compiler_binary),) + argv,
                           MappingProxyType(env), profile,
                           hashlib.sha256(profile.encode()).hexdigest(), publication,
                           reads, plan.contract_sha256)


def _held_directory(path):
    """Open normalized cwd with no symlink component, retaining its identity."""
    path = _path(str(path))
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        for component in path.parts[1:]:
            child = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


def _close_extra_descriptors():
    """Darwin dedicated single-thread launcher only; no agent Python hooks.

    /dev/fd must be the qualified OS descriptor namespace. No guessed rlimit cap
    or best-effort close: unknown entries, excess descriptors, or residue refuse.
    Runtime/Python startup and native-thread absence are qualification obligations.
    """
    names = os.listdir('/dev/fd')
    _require(len(names) <= MAX_FDS and all(n.isascii() and n.isdigit() for n in names),
             "unknown or oversized descriptor inventory")
    for fd in sorted({int(n) for n in names if int(n) > 2}, reverse=True):
        try:
            os.close(fd)
        except OSError as error:
            # listdir's own already-closed directory FD may appear in its listing.
            if error.errno != errno.EBADF:
                raise
    remaining = os.listdir('/dev/fd')
    _require(len(remaining) <= MAX_FDS and all(n.isascii() and n.isdigit() for n in remaining),
             "unknown descriptor residue")
    for name in remaining:
        fd = int(name)
        if fd > 2:
            try:
                os.fstat(fd)
            except OSError as error:
                if error.errno == errno.EBADF:
                    continue
                raise
            raise AuthorityRefusal("descriptor survived child closure")
    # Standard descriptors are the qualified private pipe endpoints, not files
    # inherited from the agent. The caller/qualifier must establish their origin.
    _require(stat.S_ISCHR(os.fstat(0).st_mode), "closed stdin device required")
    for fd in (1, 2):
        _require(stat.S_ISFIFO(os.fstat(fd).st_mode), "private stdout/stderr pipes required")


def _exec_transition(transition):
    """Irreversible internal child entry; no authority or receipt is minted.

    The protected entry reconstructs this transition from a preflighted contract,
    never deserializes a ChildTransition. Qualified startup must exclude injected
    Python hooks/native threads and establish actual private stdio endpoint origin.
    """
    _require(type(transition) is ChildTransition, "child transition required")
    _require(os.uname().sysname == 'Darwin', "Darwin child profile required")
    _require(hashlib.sha256(transition.profile.encode()).hexdigest() ==
             transition.profile_sha256, "child profile drift")
    for entry in transition.inventory:
        raw, info = _read(entry.path, entry.size, protected=True)
        _require(len(raw) == entry.size and hashlib.sha256(raw).hexdigest() == entry.sha256,
                 "protected child dependency drift")
        if entry.path in (transition.compiler, transition.sandbox_launcher):
            _require(info.st_mode & 0o111 != 0, "protected executable mode required")
    cwd = _held_directory(transition.cwd)
    try:
        # Exclusive publication ownership is acquired, not assumed from a path.
        # No pre-existing directory is adopted or cleared on refusal.
        os.mkdir(transition.publication_directory.name, mode=0o700, dir_fd=cwd)
        os.fchdir(cwd)
    finally:
        os.close(cwd)
    os.umask(0o077)
    null_fd = os.open('/dev/null', os.O_RDONLY | os.O_NOFOLLOW)
    try:
        _require(stat.S_ISCHR(os.fstat(null_fd).st_mode), "closed stdin device required")
        os.dup2(null_fd, 0)
    finally:
        if null_fd != 0:
            os.close(null_fd)
    _close_extra_descriptors()
    # Exec keeps the runtime-owned child PID. Inline policy avoids a mutable
    # per-request profile path or privileged profile-file generation service.
    os.execve(str(transition.sandbox_launcher),
              (str(transition.sandbox_launcher), '-p', transition.profile) + transition.argv,
              dict(transition.environment))
    raise AuthorityRefusal("child exec unexpectedly returned")


def execute_child(contract_path, contract_sha256, argv, input_files,
                  publication_directory, sandbox_launcher):
    """Protected launcher entry intended below trusted command/exec ONLY.

    This low-level OS transition is not callable as runner execution authority.
    Calling it locally cannot produce trusted broker evidence. Before enabling
    Session's wrapper route, an actual grant must bind this protected code and
    interpreter, dependency closure, actual policy inclusion and no-fork lifecycle.
    No boolean, static receipt or caller assertion bypasses that Session gate.
    """
    from compiler_execution_authority import preflight
    plan = preflight(Path(contract_path), contract_sha256)
    transition = prepare_transition(plan, argv, input_files, publication_directory,
                                    sandbox_launcher)
    _exec_transition(transition)


@dataclass(frozen=True)
class DirectChildExit:
    """Actual direct-process observation, explicitly NOT whole-child authority."""
    process_id: str
    exit_code: int
    stdout: bytes
    stderr: bytes
    whole_child_settlement: str = field(default='UNQUALIFIED_NO_FORK_ENFORCEMENT', init=False)


def observe_direct_exit(process_id, response, stdout, stderr):
    """Normalize trusted-connection direct exit without inventing descendants.

    The existing Session supplies its actual validated final command/exec result
    after all bounded stream deltas. Only a future live qualifier with kernel
    no-fork proof can promote this to complete settlement; no setter exists here.
    """
    _require(type(process_id) is str and 0 < len(process_id) <= 128 and
             all(c.isascii() and (c.isalnum() or c in '-_') for c in process_id),
             "owned process identifier required")
    _require(type(response) is dict and set(response) == {'exitCode', 'stdout', 'stderr'} and
             type(response['exitCode']) is int and -(2**31) <= response['exitCode'] < 2**31 and
             response['stdout'] == '' and response['stderr'] == '',
             "actual streamed command response required")
    _require(type(stdout) is bytes and type(stderr) is bytes and
             len(stdout) <= 512 and len(stderr) <= 512, "bounded actual streams required")
    return DirectChildExit(process_id, response['exitCode'], stdout, stderr)


def main(arguments=None):
    """Closed protected-child CLI, not a trusted runner/broker command.

    The new backend does not yet admit this wrapper in Session. A future actual
    qualifier must bind the protected interpreter/module and exact command shape.
    Raw caller request bytes are requests only and cannot mint authority.
    """
    import sys
    from compiler_runtime_protocol import decode, closed
    args = sys.argv[1:] if arguments is None else arguments
    _require(type(args) is list and len(args) == 6 and
             args[0] == '--contract' and args[2] == '--contract-sha256' and
             args[4] == '--request-json' and
             all(type(a) is str and '\0' not in a for a in args),
             "closed child entry arguments required")
    _require(len(args[5].encode()) <= 65536, "child request exceeds bound")
    request = closed(decode(args[5].encode()),
                     ['argv', 'input_files', 'publication_directory', 'sandbox_launcher'])
    _require(type(request['argv']) is list and type(request['input_files']) is list,
             "child argument/input arrays required")
    execute_child(Path(args[1]), args[3], tuple(request['argv']),
                  tuple(request['input_files']), request['publication_directory'],
                  request['sandbox_launcher'])


if __name__ == '__main__':
    import sys
    try:
        main()
    except (AuthorityRefusal, ValueError, OSError) as error:
        # No trusted success or origin record is emitted on child setup failure.
        sys.stderr.write('compiler child refused: ' + str(error) + '\n')
        raise SystemExit(126)
