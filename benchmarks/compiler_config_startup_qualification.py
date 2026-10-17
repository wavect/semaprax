"""Actual-startup evaluation building blocks, never an enforcement grant.

The qualification runner owns private startup/initialize/config-read transport,
wire request IDs, complete raw transcripts and process/FD identity. This module
validates the exact params/result and protected configuration snapshots. Passing
caller-supplied response bytes or snapshot objects establishes no execution origin.
"""
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path

import compiler_execution_authority as original
from compiler_configuration_provisioning import validate_user_home_plan
from compiler_effective_policy import resolve_owned_config, ResolvedConfig
from compiler_runtime_protocol import decode, encode, _validate

_SCHEMA_PINS = {
    'ConfigReadParams.json': '257c54a423b47c1d209ff1076765a1564d82322fd5161670fd489a2874de1bac',
    'ConfigReadResponse.json': '4ec77e1a3eed746037149799c3b6bb8fe7b4b593e5930fc9f410e6282de9df50',
}
MAX_CONFIGURATION = 262144


def _canonical_sha(value):
    # Keep the existing bounded/finite encoder at the boundary, then normalize
    # object order only for this separate semantic representation binding.
    parsed = decode(encode(value))
    return hashlib.sha256(json.dumps(parsed, sort_keys=True, separators=(',', ':'),
                                    allow_nan=False).encode()).hexdigest()


def _validate_retained(name, value):
    path = Path(__file__).with_name('compiler_runtime_v1') / 'config-startup' / name
    # No-follow component traversal and bounded complete-file read; the source
    # hash is compatibility custody, not root protection or execution authority.
    raw, _ = original._read(path, 65536)
    original._require(hashlib.sha256(raw).hexdigest() == _SCHEMA_PINS[name],
                      'startup protocol schema source drift')
    document = decode(raw)
    _validate(value, document, document)


def startup_request(plan):
    validate_user_home_plan(plan)
    params = {'cwd': str(plan.workspace_root), 'includeLayers': True}
    _validate_retained('ConfigReadParams.json', params)
    return params


@dataclass(frozen=True)
class StartupObservation:
    """Validated candidate bytes; no field can activate authority admission."""
    contract_sha256: str
    runtime_sha256: str
    request_params_sha256: str
    response_result_sha256: str
    configuration_sha256: str
    resolved: ResolvedConfig
    scope: str = 'CONFIG_BYTES_VALIDATED_TRANSPORT_AND_ENFORCEMENT_UNQUALIFIED'


def evaluate_startup(plan, *, request, response, held_config_bytes, model, effort):
    expected = startup_request(plan)
    original._require(type(request) is dict and set(request) == set(expected)
                      and request == expected and type(request['includeLayers']) is bool,
                      'startup config/read request differs')
    _validate_retained('ConfigReadResponse.json', response)
    config = validate_user_home_plan(plan)
    resolved = resolve_owned_config(plan, response, held_config_bytes, config,
                                    model=model, effort=effort)
    # Full request/result encodings are bound independently. The runner must
    # retain actual wire frames as well; canonical encoding is not a wire receipt.
    return StartupObservation(plan.contract_sha256, plan.runtime_sha256,
        _canonical_sha(request), _canonical_sha(response),
        resolved.configuration_sha256, resolved)


@dataclass(frozen=True)
class ConfigurationSnapshot:
    """A protected-read observation, not a held descriptor or lifetime lease."""
    contract_sha256: str
    path: Path
    data: bytes
    sha256: str
    identity: tuple[int, ...]


def acquire_configuration(plan):
    path = validate_user_home_plan(plan)
    entry = next(e for e in plan.inventory if e.path == path and e.role == 'configuration')
    original._require(type(entry.size) is int and 0 < entry.size <= MAX_CONFIGURATION,
                      'bounded startup configuration')
    raw, info = original._read(path, entry.size, protected=True)
    digest = hashlib.sha256(raw).hexdigest()
    original._require(len(raw) == entry.size and digest == entry.sha256,
                      'startup named configuration drift')
    identity = tuple(getattr(info, field) for field in
        ('st_dev', 'st_ino', 'st_size', 'st_mtime_ns', 'st_ctime_ns', 'st_mode', 'st_uid', 'st_gid'))
    return ConfigurationSnapshot(plan.contract_sha256, path, raw, digest, identity)


def require_same_configuration(before, after):
    """Endpoint continuity control; never a proof of protection between reads."""
    original._require(type(before) is ConfigurationSnapshot and type(after) is ConfigurationSnapshot,
                      'configuration snapshots required')
    for value in (before, after):
        original._require(type(value.data) is bytes and 0 < len(value.data) <= MAX_CONFIGURATION
            and type(value.identity) is tuple and len(value.identity) == 8
            and all(type(n) is int and n >= 0 for n in value.identity)
            and value.identity[2] == len(value.data)
            and hashlib.sha256(value.data).hexdigest() == value.sha256,
            'configuration snapshot fields differ')
    original._require(before == after, 'startup configuration identity/content changed')
