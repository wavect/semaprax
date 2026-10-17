"""Static provisioning for the opt-in shared-runtime v1 configuration.

A ProvisioningPlan is not execution authority. No enforcement proof format is
recognized yet: admit always refuses, including otherwise valid static plans.
Existing compiler receipt/proxy v1 observations are deliberately untouched.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
from dataclasses import dataclass
from types import MappingProxyType
from typing import Mapping

SCHEMA = "semaprax.compiler-execution-provisioning.v1"
MAX_CONTRACT_BYTES = 262144
MAX_ARTIFACT_BYTES = 268435456
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_ENV = frozenset({"PATH", "HOME", "TMPDIR", "LANG", "LC_ALL", "TZ"})


class AuthorityRefusal(ValueError):
    """The requested provisioning or execution boundary was not established."""


def _require(condition, message):
    if not condition:
        raise AuthorityRefusal(message)


def _keys(value, expected, label):
    _require(type(value) is dict and set(value) == set(expected), label + ": fields")


def _path(value):
    _require(type(value) is str and value.startswith("/") and "\0" not in value,
             "absolute path required")
    _require(value != "/" and str(Path(value)) == value and
             all(part not in (".", "..", "") for part in value.split("/")[1:]),
             "normalized non-root path required")
    return Path(value)


def _sha(value):
    _require(type(value) is str and _SHA.fullmatch(value) is not None, "SHA256 required")
    return value


def _within(path, root):
    return path == root or root in path.parents


def _unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        _require(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def _read(path: Path, limit: int, protected=False):
    """Walk every component via held no-follow directory descriptors.

    Root ownership/modes are preliminary checks, not ACL or mount authority.
    The eventual admission boundary must additionally authenticate those facts.
    """
    _require(hasattr(os, "O_NOFOLLOW") and hasattr(os, "O_DIRECTORY"),
             "no-follow descriptor traversal unavailable")
    path = _path(str(path))
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY)
    try:
        for index, component in enumerate(path.parts[1:]):
            parent = os.fstat(fd)
            if protected:
                _require(parent.st_uid == 0 and not parent.st_mode & 0o022,
                         "artifact ancestor is not root-owned and write-protected")
            last = index == len(path.parts) - 2
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if not last:
                flags |= os.O_DIRECTORY
            next_fd = os.open(component, flags, dir_fd=fd)
            os.close(fd)
            fd = next_fd
        before = os.fstat(fd)
        _require(stat.S_ISREG(before.st_mode) and before.st_size <= limit,
                 "bounded regular file required")
        if protected:
            _require(before.st_uid == 0 and not before.st_mode & 0o022,
                     "artifact is not root-owned and write-protected")
        data = bytearray()
        while len(data) <= limit:
            block = os.read(fd, min(65536, limit + 1 - len(data)))
            if not block:
                break
            data.extend(block)
        after = os.fstat(fd)
        fields = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns,
                            s.st_ctime_ns, s.st_mode, s.st_uid, s.st_gid)
        _require(len(data) <= limit and len(data) == before.st_size and
                 fields(before) == fields(after), "file changed or exceeded bound")
        return bytes(data), before
    except OSError as error:
        raise AuthorityRefusal("no-follow file acquisition refused") from error
    finally:
        os.close(fd)


def _directory(path: Path, protected: bool):
    """Preliminary no-follow directory walk; no effective-policy claim."""
    _path(str(path))
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY)
    try:
        for component in path.parts[1:]:
            parent = os.fstat(fd)
            _require(parent.st_uid == 0 and not parent.st_mode & 0o022,
                     "directory parent is not root-owned and write-protected")
            next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                              dir_fd=fd)
            os.close(fd)
            fd = next_fd
        info = os.fstat(fd)
        if protected:
            _require(info.st_uid == 0 and not info.st_mode & 0o022,
                     "directory is not root-owned and write-protected")
    except OSError as error:
        raise AuthorityRefusal("no-follow directory acquisition refused") from error
    finally:
        os.close(fd)


@dataclass(frozen=True)
class InventoryEntry:
    path: Path
    sha256: str
    size: int
    role: str


@dataclass(frozen=True)
class ProvisioningPlan:
    runtime_binary: Path
    runtime_sha256: str
    server_argv: tuple[str, ...]
    server_cwd: Path
    server_env: Mapping[str, str]
    sandbox_policy: Mapping[str, object]
    workspace_root: Path
    command_env: Mapping[str, str | None]
    compiler_binary: Path
    compiler_sha256: str
    contract_sha256: str
    agent_writable_roots: tuple[Path, ...]
    evidence_root: Path
    inventory: tuple[InventoryEntry, ...]
    # These are explicit unresolved obligations, never qualified booleans.
    unresolved: tuple[str, ...] = (
        "effective agent/command policy and managed configuration",
        "filesystem ACL/mount and ancestor rename authority",
        "complete dynamic loader/dependency closure",
        "network and Unix socket enforcement",
        "closed server-computed environment continuity",
    )


def _parse(raw: bytes, digest: str):
    _require(hashlib.sha256(raw).hexdigest() == _sha(digest), "contract hash drift")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_pairs,
                           parse_constant=lambda _: (_ for _ in ()).throw(
                               AuthorityRefusal("nonfinite JSON")))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise AuthorityRefusal("invalid contract JSON") from error
    _keys(value, {"schema", "runtime_binary", "compiler_binary", "inventory",
                  "server_cwd", "workspace_root", "evidence_root", "agent_writable_roots",
                  "server_env", "command_env", "sandbox_policy", "approval_policy",
                  "alternate_execution", "enforcement_proof"}, "contract")
    _require(value["schema"] == SCHEMA, "unsupported provisioning version")
    _require(value["approval_policy"] == "never" and
             value["alternate_execution"] == "disabled", "execution authority widened")
    _require(value["enforcement_proof"] is None, "unrecognized enforcement proof")
    roots_raw = value["agent_writable_roots"]
    _require(type(roots_raw) is list and 0 < len(roots_raw) <= 64, "writable root inventory")
    roots = tuple(_path(p) for p in roots_raw)
    _require(len(set(roots)) == len(roots), "duplicate writable root")
    workspace, cwd, evidence = (_path(value[k]) for k in
                                ("workspace_root", "server_cwd", "evidence_root"))
    _require(workspace in roots and cwd == workspace, "fixed workspace/cwd required")
    _require(not any(_within(evidence, root) or _within(root, evidence) for root in roots),
             "evidence overlaps writable scope")
    policy = value["sandbox_policy"]
    _keys(policy, {"type", "writableRoots", "networkAccess", "excludeSlashTmp",
                   "excludeTmpdirEnvVar"}, "sandbox policy")
    _require(policy == {"type": "workspaceWrite", "writableRoots": roots_raw,
                        "networkAccess": False, "excludeSlashTmp": True,
                        "excludeTmpdirEnvVar": True} and
             type(policy["networkAccess"]) is bool and
             type(policy["excludeSlashTmp"]) is bool and
             type(policy["excludeTmpdirEnvVar"]) is bool, "fixed matched policy required")
    env = value["server_env"]
    _keys(env, _ENV, "closed server environment")
    _require(all(type(v) is str and "\0" not in v for v in env.values()), "environment string")
    _require(env["LANG"] == "C" and env["LC_ALL"] == "C" and env["TZ"] == "UTC",
             "fixed locale required")
    for key in ("PATH", "HOME", "TMPDIR"):
        _path(env[key])  # PATH is one protected directory, never ambient search.
        _require(not any(_within(Path(env[key]), root) for root in roots), "mutable launch environment")
    overrides = value["command_env"]
    _keys(overrides, _ENV, "command environment")
    _require(overrides == env, "command must retain exact declared launch environment")
    rows = value["inventory"]
    _require(type(rows) is list and 2 <= len(rows) <= 1024, "full declared inventory required")
    entries = []
    for row in rows:
        _keys(row, {"path", "sha256", "size", "role"}, "artifact")
        path, sha = _path(row["path"]), _sha(row["sha256"])
        _require(type(row["size"]) is int and 0 < row["size"] <= MAX_ARTIFACT_BYTES,
                 "artifact size")
        _require(row["role"] in ("runtime", "compiler", "dependency", "configuration", "schema"),
                 "artifact role")
        _require(not any(_within(path, root) or _within(root, path) for root in roots),
                 "artifact overlaps writable scope")
        _require(path.name == sha, "content-addressed artifact basename required")
        entries.append(InventoryEntry(path, sha, row["size"], row["role"]))
    _require(len({e.path for e in entries}) == len(entries), "duplicate artifact")
    _require(all(any(e.role == role for e in entries) for role in ("configuration", "schema")),
             "configuration and schema inventory required")
    runtime, compiler = _path(value["runtime_binary"]), _path(value["compiler_binary"])
    def executable(path, role):
        matches = [e for e in entries if e.path == path and e.role == role]
        _require(len(matches) == 1 and sum(e.role == role for e in entries) == 1,
                 "exact executable inventory role required")
        return matches[0].sha256
    runtime_sha, compiler_sha = executable(runtime, "runtime"), executable(compiler, "compiler")
    # No arbitrary server flags/config overrides accepted by this first version.
    argv = (str(runtime), "app-server", "--listen", "stdio://")
    frozen_policy = dict(policy, writableRoots=tuple(roots_raw))
    return ProvisioningPlan(runtime, runtime_sha, argv, cwd, MappingProxyType(dict(env)),
                            MappingProxyType(frozen_policy), workspace,
                            MappingProxyType(dict(overrides)), compiler, compiler_sha,
                            digest, roots, evidence, tuple(entries))


def preflight(contract_path: Path, expected_sha256: str) -> ProvisioningPlan:
    """Validate declared static provisioning; does not certify enforcement."""
    raw, _ = _read(contract_path, MAX_CONTRACT_BYTES, protected=True)
    plan = _parse(raw, expected_sha256)
    # Candidate contents may be writable; its parent/ancestors must not permit
    # replacing the candidate directory itself. Mode checks cannot prove ACLs.
    for root in plan.agent_writable_roots:
        _directory(root, protected=False)
    _directory(plan.evidence_root, protected=True)
    for key in ("PATH", "HOME", "TMPDIR"):
        _directory(Path(plan.server_env[key]), protected=True)
    for entry in plan.inventory:
        data, metadata = _read(entry.path, entry.size, protected=True)
        _require(len(data) == entry.size and hashlib.sha256(data).hexdigest() == entry.sha256,
                 "artifact inventory drift")
        if entry.role in ("runtime", "compiler"):
            _require(metadata.st_mode & 0o111 != 0, "executable mode missing")
    return plan


class ExecutionAuthority:
    """Reserved live authority type. This version cannot construct a grant."""
    def __new__(cls, *args, **kwargs):
        raise AuthorityRefusal("no recognized effective-enforcement proof implementation")

    def check_current(self):
        raise AuthorityRefusal("execution authority unavailable")

    def check_private_path(self, path: Path):
        raise AuthorityRefusal("private path authority unavailable")

    def authorize_cwd(self, cwd: str):
        raise AuthorityRefusal("cwd authority unavailable")


def admit(contract_path: Path, expected_sha256: str) -> ExecutionAuthority:
    preflight(contract_path, expected_sha256)
    raise AuthorityRefusal("static provisioning cannot authenticate effective enforcement")
