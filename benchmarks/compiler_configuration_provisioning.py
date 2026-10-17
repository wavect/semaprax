"""Closed explicit CODEX_HOME provisioning extension; static plans only.

The complete v1 parser/admission remains unchanged. This v2 supports exactly one
named user config required by the pinned runtime's retained loading schema.
"""
from dataclasses import replace
import hashlib
import json
from pathlib import Path
from types import MappingProxyType

import compiler_execution_authority as original

SCHEMA = 'semaprax.compiler-execution-provisioning.user-home.v2'
RUNTIME_SHA256 = '1afb28c01d7d49e892bd75f11076b10f1451e34e0ee308a95e5ab7f69f9c84fe'
CONFIG_SCHEMA_SHA256 = '4ec77e1a3eed746037149799c3b6bb8fe7b4b593e5930fc9f410e6282de9df50'
APP_SERVER_HELP_SHA256 = '4226dbf080b9d3a620385fba067bd3b8ac3c76654f28c3bd4d92ae20df2b03d2'
_ENV = {'PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'TZ', 'CODEX_HOME'}


def _loader(plan):
    original._require(type(plan) is original.ProvisioningPlan, 'static plan required')
    env = plan.server_env
    original._require(set(env) == _ENV and dict(env) == dict(plan.command_env),
                      'explicit CODEX_HOME environment required')
    home = original._path(env['HOME'])
    codex_home = original._path(env['CODEX_HOME'])
    original._require(codex_home == home / '.codex', 'one protected HOME/.codex layout')
    config = codex_home / 'config.toml'
    original._require(plan.runtime_sha256 == RUNTIME_SHA256, 'unsupported runtime loader')
    original._require(sum(e.role == 'schema' and e.sha256 == CONFIG_SCHEMA_SHA256
                          for e in plan.inventory) == 1, 'pinned config schema inventory required')
    rows = [e for e in plan.inventory if e.role == 'configuration']
    original._require(len(rows) == 1 and rows[0].path == config,
                      'exact named user config inventory required')
    return codex_home, config


def validate_user_home_plan(plan):
    """Structural consistency only; no filesystem/policy/grant claim."""
    codex_home, config = _loader(plan)
    original._require(plan.agent_writable_roots == (plan.workspace_root,)
                      and plan.server_cwd == plan.workspace_root, 'one fixed user-home trial root')
    original._require(not any(original._within(codex_home, r) or original._within(r, codex_home)
                               for r in plan.agent_writable_roots), 'mutable config home')
    return config


def parse_user_home(raw, expected_sha256):
    original._require(type(raw) is bytes and 0 < len(raw) <= original.MAX_CONTRACT_BYTES,
                      'bounded versioned contract')
    original._require(hashlib.sha256(raw).hexdigest() == original._sha(expected_sha256),
                      'versioned contract hash drift')
    try:
        value = json.loads(raw.decode('utf-8'), object_pairs_hook=original._unique_pairs,
                           parse_constant=lambda _: original._require(False, 'nonfinite contract'))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise original.AuthorityRefusal('versioned contract JSON') from error
    original._require(type(value) is dict and value.get('schema') == SCHEMA,
                      'unsupported user-home contract')
    original._require('configuration_loader' in value, 'explicit loader binding required')
    loader = value['configuration_loader']
    original._keys(loader, {'kind', 'codex_home', 'config_path', 'runtime_sha256',
                            'config_schema_sha256', 'app_server_help_sha256'}, 'loader')
    original._require(loader['kind'] == 'explicit-codex-home.v1'
                      and loader['runtime_sha256'] == RUNTIME_SHA256
                      and loader['config_schema_sha256'] == CONFIG_SCHEMA_SHA256
                      and loader['app_server_help_sha256'] == APP_SERVER_HELP_SHA256,
                      'unsupported installed loader evidence')
    for key in ('server_env', 'command_env'):
        original._keys(value.get(key), _ENV, 'versioned closed environment')
    original._require(value['server_env'] == value['command_env'], 'versioned environment differs')
    env = value['server_env']
    codex_home = original._path(env['CODEX_HOME'])
    original._require(codex_home == original._path(env['HOME']) / '.codex'
                      and loader['codex_home'] == str(codex_home)
                      and loader['config_path'] == str(codex_home / 'config.toml'),
                      'explicit loader path differs')
    rows = value.get('inventory')
    original._require(type(rows) is list and len(rows) <= 1024, 'bounded inventory')
    for row in rows:
        original._keys(row, {'path', 'sha256', 'size', 'role'}, 'versioned artifact')
    configs = [r for r in rows if type(r) is dict and r.get('role') == 'configuration']
    original._require(len(configs) == 1 and configs[0].get('path') == loader['config_path'],
                      'exact named configuration row')
    original._require(type(configs[0]['size']) is int and
                      0 < configs[0]['size'] <= original.MAX_CONTRACT_BYTES,
                      'bounded named configuration size')
    # Finite adapter to the complete original v1 validation. No real file read
    # uses the synthetic content-addressed sibling. All other fields/rows remain.
    old = json.loads(raw.decode('utf-8'))
    del old['configuration_loader']
    old['schema'] = original.SCHEMA
    for key in ('server_env', 'command_env'):
        del old[key]['CODEX_HOME']
    for row in old['inventory']:
        if row['role'] == 'configuration':
            row['path'] = str(codex_home / original._sha(row['sha256']))
    old_raw = json.dumps(old, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    plan = original._parse(old_raw, hashlib.sha256(old_raw).hexdigest())
    entries = tuple(replace(e, path=codex_home / 'config.toml')
                    if e.role == 'configuration' else e for e in plan.inventory)
    original._require(len({e.path for e in entries}) == len(entries), 'restored inventory collision')
    plan = replace(plan, server_env=MappingProxyType(dict(env)),
                   command_env=MappingProxyType(dict(value['command_env'])), inventory=entries,
                   contract_sha256=expected_sha256)
    validate_user_home_plan(plan)
    return plan


def preflight_user_home(contract_path, expected_sha256):
    """Acquire the actual named file via old protected no-follow reader."""
    raw, _ = original._read(Path(contract_path), original.MAX_CONTRACT_BYTES, protected=True)
    plan = parse_user_home(raw, expected_sha256)
    for root in plan.agent_writable_roots:
        original._directory(root, protected=False)
    original._directory(plan.evidence_root, protected=True)
    for key in ('PATH', 'HOME', 'TMPDIR', 'CODEX_HOME'):
        original._directory(Path(plan.server_env[key]), protected=True)
    for entry in plan.inventory:
        data, metadata = original._read(entry.path, entry.size, protected=True)
        original._require(len(data) == entry.size and hashlib.sha256(data).hexdigest() == entry.sha256,
                          'versioned inventory byte drift')
        if entry.role in ('runtime', 'compiler'):
            original._require(metadata.st_mode & 0o111 != 0, 'executable mode missing')
    return plan
