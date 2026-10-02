#!/usr/bin/env python3
"""Native Claude Team transport over the existing bounded pilot gateway."""
from __future__ import annotations
import argparse
import base64
import fcntl
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import sys
import stat
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'benchmarks/cross-language-v1'))
from agent import claude_subscription as native
from agent import pilot_protocol as common
from opencode_agent_task_pilot.review_workflow import _read_regular, FROZEN_MANIFEST_SHA256

spec = importlib.util.spec_from_file_location('claude_pilot_host', ROOT / 'scripts/opencode-agent-task-pilot.py')
pilot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pilot)
SCHEMA = 'semaprax.claude-agent-task-pilot-protocol.v1'
RECORD = 'semaprax.claude-agent-task-pilot.v1'
MODELS = [
    {'id': 'haiku45', 'provider': 'anthropic', 'model': 'claude-haiku-4-5-20251001',
     'revision': 'claude-haiku-4-5-20251001', 'configured_model': 'anthropic/claude-haiku-4-5-20251001',
     'usage_key': 'claude-haiku-4-5-20251001', 'canonical_model': 'claude-haiku-4-5'},
    {'id': 'sonnet55', 'provider': 'anthropic', 'model': 'claude-sonnet-5-5',
     'revision': 'claude-sonnet-5-5', 'configured_model': 'anthropic/claude-sonnet-5-5',
     'usage_key': 'claude-sonnet-5-5', 'canonical_model': 'claude-sonnet-5-5'},
]
CAPS = {'seconds': 300, 'max_turns': 32, 'max_prompt_bytes': 65536,
        'max_stream_bytes': 1048576, 'max_reported_tokens': 131072, 'max_cache_read_tokens': 1048576,
        'max_estimated_api_usd': 0.25, 'cohort_max_estimated_api_usd': 9.0, 'agent_retries': 0}
ISOLATION_FLAGS = ['--restricted', '--strict-mcp-config', '--setting-sources', '',
                   '--disable-slash-commands', '--settings',
                   json.dumps({'disableAllHooks': True, 'claudeMdExcludes': ['**'], 'autoMemoryEnabled': False}, sort_keys=True)]
SYSTEM = ('Complete the supplied task only through mcp__semaprax__command. '
          'Use its --help first for the exact admitted command syntax. No shell or other tools. '
          'Preserve stable identities, evaluation order and unrelated edits. Do not publish. '
          'Finish with validation, review artifacts and explicit analysis blind spots.')


def canonical(value):
    return common.canonical(value)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write(path, value):
    pilot.exclusive_write(Path(path), canonical(value))


def implementation():
    files = [ROOT / 'scripts/opencode-agent-task-pilot.py', ROOT / 'scripts/agent-task-comparison-runner.py',
             ROOT / 'scripts/agent-task-comparison.py', Path(__file__).resolve()]
    files += sorted((ROOT / 'scripts/opencode_agent_task_pilot').glob('*.py'))
    files += [ROOT / 'benchmarks/cross-language-v1/agent' / name for name in ('claude_subscription.py', 'pilot_protocol.py')]
    return {str(path.relative_to(ROOT)): sha(_read_regular(path, 1024 * 1024)) for path in files}



def require_private_directory(path):
    path = Path(path)
    facts = path.lstat()
    if (path != path.resolve() or not stat.S_ISDIR(facts.st_mode)
            or facts.st_uid != os.getuid() or facts.st_mode & 0o077):
        raise ValueError('private_evidence_directory_required')


def require_disjoint(*paths):
    paths = [Path(path) for path in paths]
    for index, left in enumerate(paths):
        if not left.is_absolute() or left != left.resolve():
            raise ValueError('canonical_authority_roots_required')
        for right in paths[index + 1:]:
            if left == right or left in right.parents or right in left.parents:
                raise ValueError('authority_roots_overlap')


def reserve(protocol, digest, cell_id):
    """Durable before-spawn reservation; unknown/failed delivery never refunds."""
    allowed = {f"{model['id']}-{position:02}" for model in protocol['models'] for position in range(1, 19)}
    if cell_id not in allowed:
        raise ValueError('reservation_cell_not_in_schedule')
    root = Path(protocol['authority']['evidence_root']); require_private_directory(root)
    lock = os.open(root / 'dispatch-budget.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        if not stat.S_ISREG(os.fstat(lock).st_mode) or os.fstat(lock).st_nlink != 1:
            raise ValueError('budget_lock_type_refused')
        fcntl.flock(lock, fcntl.LOCK_EX)
        path = root / 'dispatch-budget.json'
        ledger = common.strict_json(_read_regular(path, 65536)) if path.exists() else {'protocol_sha256': digest, 'cells': [], 'reported_costs': {}, 'halted': None}
        if set(ledger) != {'protocol_sha256', 'cells', 'reported_costs', 'halted'} or ledger['protocol_sha256'] != digest:
            raise ValueError('budget_protocol_mismatch')
        cells = ledger['cells']
        if (not isinstance(cells, list) or any(cell not in allowed for cell in cells)
                or len(set(cells)) != len(cells) or cell_id in cells):
            raise ValueError('budget_cell_already_reserved_or_invalid')
        if ledger['halted'] is not None or any(cell not in ledger['reported_costs'] for cell in cells):
            raise ValueError('cohort_cost_unknown_or_halted')
        # Integer microdollars avoid admitting float rounding beyond the cap.
        per = round(protocol['caps']['max_estimated_api_usd'] * 1000000)
        total = round(protocol['caps']['cohort_max_estimated_api_usd'] * 1000000)
        if (len(cells) + 1) * per > total:
            raise ValueError('cohort_budget_exhausted')
        ledger['cells'] = cells + [cell_id]
        fd, pending = tempfile.mkstemp(prefix='.budget-', dir=root)
        try:
            with os.fdopen(fd, 'wb') as stream:
                stream.write(canonical(ledger)); stream.flush(); os.fsync(stream.fileno())
            os.replace(pending, path)
            directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try: os.fsync(directory)
            finally: os.close(directory)
        finally:
            if os.path.exists(pending): os.unlink(pending)
        return {'cell_id': cell_id, 'reserved_api_equivalent_micro_usd': per,
                'cohort_reserved_micro_usd': len(ledger['cells']) * per,
                'cohort_cap_micro_usd': total, 'refund': 'never'}
    finally:
        os.close(lock)


def settle_cost(protocol, digest, cell_id, cost, admission_failure=None):
    root = Path(protocol['authority']['evidence_root']); require_private_directory(root)
    lock = os.open(root / 'dispatch-budget.lock', os.O_RDWR | os.O_NOFOLLOW)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX)
        path = root / 'dispatch-budget.json'; ledger = common.strict_json(_read_regular(path, 65536))
        if ledger['protocol_sha256'] != digest or cell_id not in ledger['cells'] or cell_id in ledger['reported_costs']:
            raise ValueError('budget_settlement_binding_refused')
        valid = type(cost) in (int, float) and math.isfinite(cost) and 0 <= cost <= protocol['caps']['max_estimated_api_usd']
        ledger['reported_costs'][cell_id] = cost if valid else None
        if admission_failure is not None:
            ledger['halted'] = 'native_admission_failed_no_further_dispatch:' + admission_failure
        elif not valid:
            ledger['halted'] = 'unobserved_or_overrun_provider_cost_no_further_dispatch'
        fd, pending = tempfile.mkstemp(prefix='.budget-', dir=root)
        try:
            with os.fdopen(fd, 'wb') as stream:
                stream.write(canonical(ledger)); stream.flush(); os.fsync(stream.fileno())
            os.replace(pending, path)
            directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try: os.fsync(directory)
            finally: os.close(directory)
        finally:
            if os.path.exists(pending): os.unlink(pending)
    finally:
        os.close(lock)

def freeze(authority):
    common.exact(authority, ('claude', 'compiler', 'home', 'login', 'evidence_root', 'authorization', 'review'), 'authority_shape')
    common.exact(authority['review'], ('mode', 'authorization'), 'review_shape')
    if authority['review']['mode'] != 'operator_recorded_user_waiver' or not authority['review']['authorization']:
        raise ValueError('explicit_user_review_waiver_required')
    if not authority['authorization'] or not re.fullmatch(r'[A-Za-z0-9_.-]{1,256}', authority['login']):
        raise ValueError('explicit_subscription_authority_required')
    pins = {}
    for key in ('claude', 'compiler', 'home', 'evidence_root'):
        path = Path(authority[key])
        if not path.is_absolute() or path != path.resolve() or not path.exists():
            raise ValueError('canonical_existing_authority_path_required')
        if key in ('claude', 'compiler'):
            if not os.access(path, os.X_OK):
                raise ValueError('executable_authority_required')
            pins[key] = common.provenance.file_digest(path, 1024 * 1024 * 1024)[1]
        elif not path.is_dir():
            raise ValueError('directory_authority_required')
    home, evidence = Path(authority['home']), Path(authority['evidence_root'])
    require_private_directory(evidence)
    require_disjoint(home, evidence)
    identity = native.native_identity()
    if identity['native_platform'] != 'darwin-arm64':
        raise ValueError('current_pilot_requires_darwin_authority')
    schedule = pilot.runner.make_schedule('benchmarks/agent-task-comparison-v1/manifest.json')
    if len(schedule['rows']) != 18:
        raise ValueError('frozen_eighteen_positions_required')
    return {'schema': SCHEMA, 'authority': authority, 'pins': pins, 'host': identity,
            'runner_revision': common.runner_revision(), 'manifest_sha256': FROZEN_MANIFEST_SHA256,
            'implementation': implementation(), 'models': MODELS, 'cli_version': '2.1.286',
            'caps': CAPS, 'system_prompt': SYSTEM, 'schedule': schedule,
            'positions_per_model': 18, 'required_records': 36,
            'egress': 'frozen task plus lane-enforced compiler MCP responses; no native tools',
            'cost_kind': 'provider API-equivalent estimate; subscription invoice unknown; no billing changes'}


def load(path, expected=None):
    raw = _read_regular(path, 1024 * 1024)
    value = common.strict_json(raw)
    if canonical(value) != raw or (expected is not None and sha(raw) != expected) or value != freeze(value['authority']):
        raise ValueError('frozen_native_protocol_drift')
    return value, sha(raw)


def usage(body, model, caps):
    result = common.strict_json(body)
    if not isinstance(result, dict) or result.get('type') != 'result':
        raise ValueError('native_result_required')
    observed = result.get('modelUsage')
    if not isinstance(observed, dict) or set(observed) != {model['usage_key']}:
        raise ValueError('native_usage_key_mismatch')
    facts = observed[model['usage_key']]
    if facts.get('canonicalModel') != model['canonical_model'] or facts.get('provider') != 'firstParty':
        raise ValueError('native_model_provenance_mismatch')
    counters = result.get('usage', {})
    tokens = {name: counters.get(name) for name in ('input_tokens', 'output_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens')}
    if (any(type(value) is not int or value < 0 for value in tokens.values())
            or sum(tokens[name] for name in ('input_tokens', 'output_tokens', 'cache_creation_input_tokens')) > caps['max_reported_tokens']
            or tokens['cache_read_input_tokens'] > caps['max_cache_read_tokens']):
        raise ValueError('native_usage_bound')
    cost = result.get('total_cost_usd')
    if type(cost) not in (int, float) or not math.isfinite(cost) or not 0 <= cost <= caps['max_estimated_api_usd']:
        raise ValueError('native_cost_bound')
    turns = result.get('num_turns')
    exhausted = (result.get('subtype') == 'error_max_turns' and result.get('is_error') is True
                 and result.get('terminal_reason') == 'max_turns')
    if type(turns) is not int or not 0 < turns <= caps['max_turns'] + int(exhausted):
        raise ValueError('native_turn_bound')
    if result.get('subagent_stats', {}).get('spawned') != 0 or result.get('queued_turn_count') != 0:
        raise ValueError('native_extra_dispatch_refused')
    return {'status': 'observed', 'usage': tokens, 'estimated_api_cost_usd': cost,
            'subscription_invoice_cost_usd': None, 'model_usage': observed, 'num_turns': turns,
            'result_subtype': result.get('subtype'), 'is_error': result.get('is_error'),
            'permission_denials': result.get('permission_denials'), 'token_cap_kind': 'post_response_admission'}


class Transport:
    def __init__(self, expected_digest, cell_id=None):
        self.expected_digest = expected_digest
        self.cell_id = cell_id
        self.receipt = None

    def load_protocol(self, path):
        return load(path, self.expected_digest)

    def execute(self, protocol, model, state, candidate, profile, mcp, prompt, lane, timeout):
        caps = protocol['caps']
        if timeout != caps['seconds'] or len(prompt.encode()) + len(SYSTEM.encode()) > caps['max_prompt_bytes']:
            raise pilot.PilotFailure('native_frozen_bound_mismatch')
        authority = protocol['authority']
        require_disjoint(Path(authority['home']), Path(authority['evidence_root']), state, candidate)
        # Only the pinned native CLI receives the explicit subscription home.
        # The compiler MCP subtree gets a closed private environment and the
        # original physically probed seatbelt policy, including home denial.
        source = Path(authority['claude'])
        binary = common.provenance.read_regular(source, 256 * 1024 * 1024)
        if sha(binary) != protocol['pins']['claude']:
            raise pilot.PilotFailure('native_binary_drift')
        staged = state / 'claude'
        staged.write_bytes(binary); staged.chmod(0o500)
        env = {'HOME': authority['home'], 'USER': authority['login'], 'LOGNAME': authority['login'],
               'PATH': '/usr/bin:/bin', 'TMPDIR': str(state), 'DISABLE_AUTOUPDATER': '1',
               'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC': '1', 'ENABLE_TOOL_SEARCH': 'false'}
        for path in native.MANAGED:
            if os.path.lexists(path):
                raise pilot.PilotFailure('managed_claude_settings_refused')
        version = native.capture([str(staged), '--version'], state, env, b'', 5)
        if (version['exit_code'] or version['failure'] or
                base64.b64decode(version['stdout_base64']).decode().strip() != protocol['cli_version'] + ' (Claude Code)'):
            raise pilot.PilotFailure('native_cli_version_mismatch')
        entry = mcp['semaprax']
        clean = ['/usr/bin/env', '-i', 'PATH=/usr/bin:/bin', 'HOME=' + str(state), 'TMPDIR=' + str(state),
                 'SEMAPRAX_PILOT_GATEWAY=' + entry['environment']['SEMAPRAX_PILOT_GATEWAY'],
                 *entry['command']]
        confined = pilot.sandboxed(clean[0], profile, clean[1:])
        config = state / 'claude-mcp.json'
        config.write_bytes(canonical({'mcpServers': {'semaprax': {'type': 'stdio', 'command': confined[0], 'args': confined[1:]}}}))
        argv = [str(staged), '--print', '--output-format', 'json', '--tools', '',
                '--allowedTools', 'mcp__semaprax__command', '--no-session-persistence', *ISOLATION_FLAGS,
                '--mcp-config', str(config), '--permission-prompts', 'none',
                '--prompt-suggestions', 'false', '--max-turns', str(caps['max_turns']),
                '--max-budget-usd', str(caps['max_estimated_api_usd']), '--model', model['model'], '--system-prompt', SYSTEM]
        self.receipt = {'dispatches': 0, 'argv': argv[1:], 'native_host': protocol['host'],
                        'binary_sha256': protocol['pins']['claude'], 'prompt_sha256': sha(prompt.encode()),
                        'system_prompt_sha256': sha(SYSTEM.encode()), 'mcp_config': json.loads(config.read_bytes()),
                        'version_receipt': version, 'internal_provider_retries': 'not_observable'}
        def started():
            self.receipt['dispatches'] = 1
        self.receipt['budget_reservation'] = reserve(protocol, self.expected_digest, self.cell_id)
        captured = native.capture(argv, candidate, env, prompt.encode(), timeout,
                                  maximum=caps['max_stream_bytes'], on_started=started)
        self.receipt.update(captured)
        try:
            observed_cost = common.strict_json(base64.b64decode(captured['stdout_base64'])).get('total_cost_usd')
        except (ValueError, TypeError, AttributeError):
            observed_cost = None
        out = base64.b64decode(captured['stdout_base64']); err = base64.b64decode(captured['stderr_base64'])
        try:
            if captured['failure']:
                raise ValueError(captured['failure'])
            counters = usage(out, model, caps)
            result = common.strict_json(out)
            exhausted = (result.get('subtype') == 'error_max_turns' and result.get('is_error') is True
                         and result.get('terminal_reason') == 'max_turns')
            if captured['exit_code'] != (1 if exhausted else 0):
                raise ValueError('native_cli_failed')
            if not exhausted and (result.get('subtype') != 'success' or result.get('is_error') is not False):
                raise ValueError('native_provider_' + str(result.get('subtype')))
        except (ValueError, KeyError, TypeError, AttributeError) as error:
            settle_cost(protocol, self.expected_digest, self.cell_id, observed_cost, str(error))
            raise pilot.PilotFailure(str(error), out, err) from error
        settle_cost(protocol, self.expected_digest, self.cell_id, observed_cost)
        return out, err, out, result.get('session_id'), counters

    def classify_record(self, record, protocol):
        result = dict(record, schema=RECORD, native_transport_receipt=self.receipt,
                      human_review={'status': 'operator_asserted_user_waiver', 'authorization': protocol['authority']['review']['authorization'],
                                    'reviewer_id': None, 'active_ms': None},
                      native_protocol_revision=protocol['runner_revision'])
        reasons = [reason for reason in record['eligibility']['reasons'] if not reason.startswith('blinded active review time:')]
        if record.get('provider_usage', {}).get('status') != 'observed':
            reasons.append('native provider usage: unavailable')
        result['eligible_for_technical_scoring_under_waiver'] = not reasons
        result['technical_ineligibility_reasons'] = reasons
        if record.get('provider_usage', {}).get('result_subtype') == 'error_max_turns':
            result['outcome'] = 'failed'
            result['failure'] = record.get('failure') or 'native_provider_error_max_turns'
        # Preserve historical eligibility/status. A waiver is not a measurement.
        return result


def audit(protocol_path, expected):
    protocol, digest = load(protocol_path, expected)
    root = Path(protocol['authority']['evidence_root'])
    expected_paths = {root / f"{model['id']}-{position:02}" / 'record.json' for model in protocol['models'] for position in range(1,19)}
    if set(root.glob('*/record.json')) - expected_paths:
        raise ValueError('unexpected_native_trial_record')
    rows = []
    for model in protocol['models']:
        for index, cell in enumerate(protocol['schedule']['rows'], 1):
            path = root / f"{model['id']}-{index:02}" / 'record.json'
            row = {'model_id': model['id'], 'position': index, **cell, 'status': 'missing'}
            if path.exists():
                raw = _read_regular(path, 8 * 1024 * 1024); value = common.strict_json(raw)
                if (value.get('schema') != RECORD or value.get('protocol_sha256') != digest
                        or value.get('model_identity') != model or value.get('task') != cell['task']
                        or value.get('lane') != cell['lane'] or value.get('trial') != cell['trial']):
                    raise ValueError('native_trial_binding_mismatch')
                if (value.get('native_protocol_revision') != protocol['runner_revision']
                        or value.get('manifest_sha256') != protocol['manifest_sha256']
                        or value.get('outcome') not in ('completed','failed','aborted')):
                    raise ValueError('native_record_subject_mismatch')
                waiver = {'status':'operator_asserted_user_waiver',
                          'authorization':protocol['authority']['review']['authorization'],
                          'reviewer_id':None,'active_ms':None}
                if value.get('human_review') != waiver:
                    raise ValueError('native_review_claim_mismatch')
                for name, field in (('stdout.jsonl','stdout_sha256'),('stderr.txt','stderr_sha256'),
                                    ('session.json','session_sha256'),('gateway.jsonl','gateway_sha256'),('mcp-wire.jsonl','mcp_wire_sha256')):
                    if sha(_read_regular(path.parent/name, 32*1024*1024)) != value.get(field):
                        raise ValueError('native_primary_evidence_drift')
                if value.get('provider_usage',{}).get('status') == 'observed':
                    if usage(_read_regular(path.parent/'session.json',1048576),model,protocol['caps']) != value['provider_usage']:
                        raise ValueError('native_provider_usage_drift')
                row.update(status=value['outcome'], failure=value['failure'], record_sha256=sha(raw),
                           acceptance=value['acceptance'], provider_usage=value['provider_usage'],
                           technical_eligible=value['eligible_for_technical_scoring_under_waiver'],
                           human_review=value['human_review'])
            rows.append(row)
    return {'schema': 'semaprax.claude-agent-task-pilot-audit.v1', 'protocol_sha256': digest,
            'required_records': 36, 'rows': rows, 'complete': all(row['status'] != 'missing' for row in rows),
            'human_review': 'operator_recorded_user_waiver_not_measured', 'historical_eligibility': 'not_relabelled',
            'comparative_claim': 'bounded technical outcomes only; no human review timing or superiority claim'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('freeze', 'run', 'audit'))
    parser.add_argument('--authority'); parser.add_argument('--output')
    parser.add_argument('--protocol'); parser.add_argument('--protocol-sha256')
    parser.add_argument('--model-id'); parser.add_argument('--position', type=int)
    args = parser.parse_args()
    if args.command == 'freeze':
        value = freeze(json.loads(Path(args.authority).read_bytes())); write(args.output, value); print(sha(canonical(value))); return
    if not args.protocol_sha256 or not re.fullmatch(r'[0-9a-f]{64}', args.protocol_sha256):
        raise ValueError('explicit_protocol_digest_required')
    protocol, digest = load(args.protocol, args.protocol_sha256)
    if args.command == 'audit':
        value = audit(args.protocol, digest); write(args.output, value); print(json.dumps({'complete': value['complete']})); return
    model = next(row for row in protocol['models'] if row['id'] == args.model_id)
    if not args.position or not 1 <= args.position <= 18:
        raise ValueError('invalid_scheduled_position')
    cell = protocol['schedule']['rows'][args.position - 1]
    directory = Path(protocol['authority']['evidence_root']) / f"{model['id']}-{args.position:02}"
    if directory.exists() or directory.is_symlink():
        raise ValueError('trial_already_attempted')
    transport = Transport(digest, directory.name)
    try:
        result = pilot.run_tuple(cell['task'], cell['lane'], cell['trial'], None,
                                 protocol['authority']['compiler'], directory, protocol['caps']['seconds'],
                                 args.protocol, model['id'], transport=transport)
    except Exception:
        if not (directory / 'record.json').exists():
            raise
        result = json.loads((directory / 'record.json').read_bytes())
    print(json.dumps({'model': model['id'], 'position': args.position, 'outcome': result['outcome'],
                      'failure': result['failure'], 'record_sha256': sha((directory / 'record.json').read_bytes())}))


if __name__ == '__main__':
    main()
