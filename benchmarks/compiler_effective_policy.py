"""Finite declared-policy/configuration checks; never execution authority.

Path algebra below describes literal, normalized names only. It proves no OS
alias, ACL, loader, socket, effective runtime policy or descendant property.
No result from this module is accepted by ExecutionAuthority/admit.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import tomllib

from compiler_execution_authority import (
    AuthorityRefusal, ProvisioningPlan, _keys, _path, _require, _sha, _within,
)
import compiler_retained_receipt as retained

POLICY_SCHEMA = 'semaprax.finite-path-policy.v1'
DESCRIPTION_SCHEMA = 'semaprax.darwin-compiler-child-description.v1'
PATH_FIELDS = ('read_roots', 'read_files', 'write_roots', 'write_files',
               'deny_roots', 'exec_files', 'metadata_files')
BOOL_FIELDS = ('network', 'unix_sockets', 'spawn')


def _digest(value):
    try:
        raw = json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    except (TypeError, ValueError, RecursionError) as error:
        raise AuthorityRefusal('non-finite JSON description') from error
    _require(len(raw) <= 262144, 'bounded JSON description')
    return hashlib.sha256(raw).hexdigest()


def _paths(values):
    _require(type(values) in (list, tuple) and len(values) <= 256, 'bounded path list')
    result = tuple(_path(value) for value in values)
    _require(len(set(result)) == len(result), 'duplicate path')
    return tuple(sorted(result))


@dataclass(frozen=True)
class Policy:
    read_roots: tuple[Path, ...]
    read_files: tuple[Path, ...]
    write_roots: tuple[Path, ...]
    write_files: tuple[Path, ...]
    deny_roots: tuple[Path, ...]
    exec_files: tuple[Path, ...]
    metadata_files: tuple[Path, ...]
    network: bool
    unix_sockets: bool
    spawn: bool

    def wire(self):
        return {'schema': POLICY_SCHEMA,
                **{key: [str(p) for p in getattr(self, key)] for key in PATH_FIELDS},
                **{key: getattr(self, key) for key in BOOL_FIELDS}}


def parse_policy(value):
    _keys(value, ('schema', *PATH_FIELDS, *BOOL_FIELDS), 'finite policy')
    _require(value['schema'] == POLICY_SCHEMA, 'finite policy schema')
    _require(all(type(value[key]) is bool for key in BOOL_FIELDS), 'policy boolean')
    return Policy(*(_paths(value[key]) for key in PATH_FIELDS),
                  *(value[key] for key in BOOL_FIELDS))


def _checked(policy):
    _require(type(policy) is Policy, 'finite Policy required')
    # Frozen dataclasses are descriptions, not trust tokens. Validate even an
    # explicitly constructed instance rather than trusting its annotation.
    return parse_policy(policy.wire())


def _denied(path, policy):
    return any(_within(path, deny) for deny in policy.deny_roots)


def _allows(path, roots, files, policy):
    return not _denied(path, policy) and (path in files or any(_within(path, r) for r in roots))


def intersect_policies(left, right):
    """Exact intersection in this finite model, including subtree exclusions."""
    left, right = _checked(left), _checked(right)
    values = {}
    for mode in ('read', 'write'):
        lr, rr = getattr(left, mode + '_roots'), getattr(right, mode + '_roots')
        lf, rf = getattr(left, mode + '_files'), getattr(right, mode + '_files')
        roots = {a if _within(a, b) else b for a in lr for b in rr
                 if _within(a, b) or _within(b, a)}
        files = {f for f in (*lf, *rf) if _allows(f, lr, lf, left)
                 and _allows(f, rr, rf, right)}
        values[mode + '_roots'] = tuple(sorted(roots))
        values[mode + '_files'] = tuple(sorted(files))
    values['deny_roots'] = tuple(sorted(set(left.deny_roots + right.deny_roots)))
    values['exec_files'] = tuple(sorted(f for f in set(left.exec_files) & set(right.exec_files)
                                        if not _denied(f, left) and not _denied(f, right)))
    candidates = set(left.metadata_files + right.metadata_files)
    values['metadata_files'] = tuple(sorted(f for f in candidates
        if _allows(f, left.read_roots, left.read_files + left.metadata_files, left)
        and _allows(f, right.read_roots, right.read_files + right.metadata_files, right)))
    return Policy(**values, **{k: getattr(left, k) and getattr(right, k) for k in BOOL_FIELDS})


def is_subset(child, parent):
    """Containment of literal permissions, not actual sandbox inclusion."""
    child, parent = _checked(child), _checked(parent)
    if any(getattr(child, k) and not getattr(parent, k) for k in BOOL_FIELDS):
        return False
    for mode in ('read', 'write'):
        cr, cf = getattr(child, mode + '_roots'), getattr(child, mode + '_files')
        pr, pf = getattr(parent, mode + '_roots'), getattr(parent, mode + '_files')
        for root in cr:
            if _denied(root, child):
                continue
            if not any(_within(root, grant) for grant in pr) or _denied(root, parent):
                return False
            # A parent's hole inside a child subtree must remain a hole. A
            # file grant cannot cover an entire subtree, including its root.
            if any(_within(deny, root) and not _denied(deny, child)
                   for deny in parent.deny_roots):
                return False
        if any(not _denied(f, child) and not _allows(f, pr, pf, parent) for f in cf):
            return False
    if any(not _denied(f, child) and not _allows(
            f, parent.read_roots, parent.read_files + parent.metadata_files, parent)
           for f in child.metadata_files):
        return False
    return all(_denied(f, child) or (f in parent.exec_files and not _denied(f, parent))
               for f in child.exec_files)


@dataclass(frozen=True)
class ResolvedConfig:
    contract_sha256: str
    configuration_sha256: str
    configuration_path: Path
    normalized_config_sha256: str
    model: str
    effort: str
    workspace_root: Path
    # Canonical immutable JSON text, not a mutable caller dictionary.
    canonical_config: str


def _plan(plan):
    _require(type(plan) is ProvisioningPlan, 'ProvisioningPlan required, not authority')
    from compiler_configuration_provisioning import validate_user_home_plan
    validate_user_home_plan(plan)
    _require(plan.agent_writable_roots == (plan.workspace_root,)
             and plan.server_cwd == plan.workspace_root, 'one fixed workspace root')


def _configuration(plan, model, effort):
    expected = {'model': model, 'model_reasoning_effort': effort,
                'approval_policy': 'never', 'sandbox_mode': 'workspace-write',
                'sandbox_workspace_write': {'writable_roots': [str(plan.workspace_root)],
                    'network_access': False, 'exclude_slash_tmp': True,
                    'exclude_tmpdir_env_var': True},
                'web_search': 'disabled', 'apps': {'_default': {
                    'enabled': False, 'open_world_enabled': False, 'destructive_enabled': False}}}
    _require(type(model) is str and 0 < len(model) <= 128
             and type(effort) is str and 0 < len(effort) <= 32, 'explicit model/effort')
    return expected


def resolve_owned_config(plan, response, held_config_bytes, config_path, *, model, effort):
    """Resolve exactly one local user layer; unknown precedence is refusal.

    held_config_bytes must come from protected no-follow acquisition by the
    future qualifier. Here its complete bytes are tied to the static inventory;
    this is not a claim that caller-supplied bytes establish live protection.
    """
    _plan(plan)
    path = _path(str(config_path))
    _require(path == Path(plan.server_env['CODEX_HOME']) / 'config.toml',
             'configuration is not the explicit runtime user file')
    _require(type(held_config_bytes) is bytes and 0 < len(held_config_bytes) <= 262144,
             'bounded held configuration')
    rows = [r for r in plan.inventory if r.path == path and r.role == 'configuration']
    _require(len(rows) == 1 and rows[0].size == len(held_config_bytes)
             and rows[0].sha256 == hashlib.sha256(held_config_bytes).hexdigest(),
             'configuration inventory binding')
    try:
        held = tomllib.loads(held_config_bytes.decode('utf-8'))
    except (ValueError, UnicodeError) as error:
        raise AuthorityRefusal('configuration TOML refused') from error
    _keys(response, ('config', 'layers', 'origins'), 'config response')
    layers = response['layers']
    _require(type(layers) is list and len(layers) == 1, 'exactly one config layer required')
    layer = layers[0]
    _require(type(layer) is dict and set(layer) in (
        {'config', 'name', 'version'}, {'config', 'name', 'version', 'disabledReason'}),
        'config layer fields')
    _require(layer.get('disabledReason') is None, 'disabled layer')
    name = layer['name']
    _require(type(name) is dict and set(name) in ({'type', 'file'}, {'type', 'file', 'profile'}),
             'user layer name fields')
    _require(name['type'] == 'user' and name['file'] == str(path)
             and name.get('profile') is None, 'only owned unprofiled user layer supported')
    version = layer['version']
    _require(type(version) is str and 0 < len(version) <= 256
             and all(32 <= ord(c) <= 126 for c in version), 'bounded config version')
    config = response['config']
    # JSON equality must not conflate true with 1. Canonical JSON compares types.
    _require(_digest(config) == _digest(held) == _digest(layer['config']),
             'complete root/layer/held configuration differs')
    expected = _configuration(plan, model, effort)
    _require(_digest(config) == _digest(expected), 'unsupported configuration or unknown fields')
    origins = response['origins']
    _keys(origins, config.keys(), 'complete config origins')
    metadata = {'name': name, 'version': version}
    _require(all(_digest(v) == _digest(metadata) for v in origins.values()), 'foreign config origin')
    normalized = dict(expected)
    normalized['sandbox_workspace_write'] = dict(expected['sandbox_workspace_write'],
                                                 writable_roots=['<trial-root>'])
    return ResolvedConfig(plan.contract_sha256, rows[0].sha256, path, _digest(normalized),
                          model, effort, plan.workspace_root,
                          json.dumps(config, sort_keys=True, separators=(',', ':')))


def _candidate_profile(compiler, reads, publication):
    # Whole exact grammar, independently checked against the launcher generator
    # by composition controls. Its Darwin operation semantics remain unqualified.
    def literal(path):
        text = str(path)
        _require(all(32 <= ord(c) < 127 and c not in '\\"' for c in text),
                 'unsupported sandbox path quoting')
        return '(literal "' + text + '")'
    metadata = sorted({a for p in (*reads, publication) for a in p.parents if a != Path('/')})
    lines = ['; semaprax.darwin-compiler-child.v1', '(version 1)', '(deny default)',
             '(allow process-exec ' + literal(compiler) + ')']
    lines += ['(allow file-read* ' + literal(p) + ')' for p in sorted(reads)]
    lines += ['(allow file-read-metadata ' + literal(p) + ')' for p in metadata]
    quoted = literal(publication).removeprefix('(literal ').removesuffix(')')
    lines += ['(allow file-read* file-write* (subpath ' + quoted + '))']
    return ('\n'.join(lines) + '\n').encode()


@dataclass(frozen=True)
class ProfileDescription:
    contract_sha256: str
    common_fingerprint_sha256: str
    canonical_description: str
    intersection: Policy


def validate_profile_description(plan, resolved, description, *, profile_bytes):
    """Check a candidate transition description, without executing/authorizing it."""
    _plan(plan)
    _require(type(resolved) is ResolvedConfig and resolved.contract_sha256 == plan.contract_sha256
             and resolved.workspace_root == plan.workspace_root, 'resolved config binding')
    expected_config = _configuration(plan, resolved.model, resolved.effort)
    _require(type(resolved.canonical_config) is str and len(resolved.canonical_config) <= 262144,
             'bounded resolved configuration')
    try:
        restored_config = json.loads(resolved.canonical_config)
    except (ValueError, RecursionError) as error:
        raise AuthorityRefusal('resolved configuration JSON') from error
    normalized_config = dict(expected_config)
    normalized_config['sandbox_workspace_write'] = dict(
        expected_config['sandbox_workspace_write'], writable_roots=['<trial-root>'])
    _require(resolved.canonical_config == json.dumps(restored_config, sort_keys=True,
                 separators=(',', ':'), allow_nan=False)
             and _digest(restored_config) == _digest(expected_config)
             and resolved.normalized_config_sha256 == _digest(normalized_config)
             and sum(r.path == resolved.configuration_path and r.role == 'configuration'
                     and r.sha256 == resolved.configuration_sha256 for r in plan.inventory) == 1,
             'resolved configuration fields differ')
    fields = ('schema', 'cwd', 'compiler', 'argv', 'publication_directory', 'input_files',
              'read_files', 'metadata_files', 'environment', 'sandbox_launcher', 'profile_sha256',
              'agent_policy', 'executor_policy')
    _keys(description, fields, 'child description')
    d = description
    _require(d['schema'] == DESCRIPTION_SCHEMA and d['cwd'] == str(plan.workspace_root)
             and d['compiler'] == str(plan.compiler_binary), 'fixed compiler/cwd')
    argv = d['argv']
    _require(type(argv) is list and len(argv) <= 34 and all(type(a) is str and '\0' not in a for a in argv)
             and sum(len(a.encode()) for a in argv) <= 16384 and len(argv) >= 2
             and argv[-2] == '--receipt-stdout-seal'
             and re.fullmatch('[0-9a-f]{64}', argv[-1]) is not None, 'closed sealed argv')
    values = retained.options(argv[:-2])
    _require(values is not None and values['project'] == '.', 'retained project grammar')
    root = plan.workspace_root
    publication = _path(d['publication_directory'])
    _require(publication.parent == root and publication != root, 'exclusive publication child')
    def relative(raw):
        _require(type(raw) is str and not raw.startswith('/') and raw != ''
                 and all(x not in ('', '.', '..') for x in raw.split('/')), 'literal relative path')
        return _path(str(root / raw))
    output, source = relative(values['--output']), relative(values['--source'])
    _require(output.parent == publication, 'output publication binding')
    inputs = _paths(d['input_files'])
    _require(source in inputs and root / 'semaprax.toml' in inputs and len(inputs) > 0
             and all(_within(p, root) and not _within(p, publication) for p in inputs),
             'captured input scope')
    inventory = {r.path for r in plan.inventory}
    reads = _paths(d['read_files'])
    _require(set(reads) == inventory | set(inputs), 'complete exact inventory/input reads')
    launcher = _path(d['sandbox_launcher'])
    _require(sum(r.path == launcher and r.role == 'dependency' for r in plan.inventory) == 1
             and launcher != plan.compiler_binary, 'protected launcher dependency')
    _sha(d['profile_sha256'])
    _require(type(profile_bytes) is bytes and 0 < len(profile_bytes) <= 262144
             and hashlib.sha256(profile_bytes).hexdigest() == d['profile_sha256'],
             'complete bounded profile bytes binding')
    _require(type(d['environment']) is dict and d['environment'] == dict(plan.command_env)
             and all(type(k) is str and type(v) is str and '\0' not in v
                     for k, v in d['environment'].items()), 'complete closed exec environment')
    agent, executor = parse_policy(d['agent_policy']), parse_policy(d['executor_policy'])
    _require(not any(getattr(executor, k) for k in BOOL_FIELDS), 'executor ambient capability')
    metadata = tuple(sorted({parent for p in (*reads, publication)
                             for parent in p.parents if parent != Path('/')}))
    _require(_paths(d['metadata_files']) == metadata, 'complete literal metadata ancestors')
    _require(not executor.deny_roots, 'candidate profile has no deny-hole rules')
    _require(executor.write_roots == (publication,) and not executor.write_files
             and executor.read_roots == (publication,) and set(executor.read_files) == set(reads)
             and executor.metadata_files == metadata
             and executor.exec_files == (plan.compiler_binary,), 'exact executor grants')
    _require(profile_bytes == _candidate_profile(plan.compiler_binary, reads, publication),
             'profile bytes differ from complete closed candidate grammar')
    _require(not any(_denied(p, executor) for p in (*reads, *metadata, publication, plan.compiler_binary)),
             'executor excludes a required resource')
    _require(is_subset(executor, agent), 'declared executor policy is wider than agent policy')
    intersection = intersect_policies(agent, executor)
    def role(path):
        if path == str(resolved.configuration_path):
            return '<owned-user-config>'
        p = Path(path)
        return '<trial-root>/' + str(p.relative_to(root)) if _within(p, root) else path
    normalized = dict(d)
    normalized['cwd'] = '<trial-root>'
    # Conservative candidate fingerprint: raw profile SHA and nonce remain.
    # Distinct dynamic profiles/nonces can therefore differ even when their
    # declared policy shape matches. No profile-equivalence or effective-policy
    # fingerprint is claimed; a future qualifier must establish that boundary.
    normalized['publication_directory'] = role(str(publication))
    normalized['input_files'] = [role(str(p)) for p in inputs]
    normalized['read_files'] = [role(str(p)) for p in reads]
    normalized['metadata_files'] = [role(str(p)) for p in metadata]
    for key in ('agent_policy', 'executor_policy'):
        normalized[key] = {k: [role(p) for p in v] if k in PATH_FIELDS else v
                           for k, v in d[key].items()}
    fingerprint = _digest({'description': normalized, 'config': resolved.normalized_config_sha256,
                           'runtime': plan.runtime_sha256, 'compiler': plan.compiler_sha256,
                           'inventory': [(str(r.path), r.sha256, r.size, r.role)
                                         for r in plan.inventory if r.role != 'configuration']})
    return ProfileDescription(plan.contract_sha256, fingerprint,
                              json.dumps(d, sort_keys=True, separators=(',', ':')), intersection)
