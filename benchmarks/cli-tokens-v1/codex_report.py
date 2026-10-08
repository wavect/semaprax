#!/usr/bin/env python3
"""Read-only recount of saved matched Codex attempts; never invokes a model."""
from __future__ import annotations
import argparse
import hashlib
import json
import statistics
from pathlib import Path
import codex_campaign as campaign


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def summarize(data: dict, rows: list[dict]) -> dict:
    order = data['campaign']['trial_order']
    require(len(rows) <= len(order), 'more attempts than planned')
    counts = {arm: 0 for arm in campaign.ARMS}
    for row, expected in zip(rows, order):
        counts[expected] += 1
        require((row['arm'], row['number']) == (expected, counts[expected]), 'attempt order differs from plan')
    arms = {}
    for arm in campaign.ARMS:
        attempted = [row for row in rows if row['arm'] == arm]
        accepted = sum(row['status'] == 'accepted' for row in attempted)
        costs = [row['conditional_api_equivalent_usd'] for row in attempted]
        total_cost = sum(costs) if costs and all(cost is not None for cost in costs) else None
        def metric(key):
            values = [row[key] for row in attempted]
            return {'total': sum(values), 'mean_per_attempt': statistics.mean(values),
                    'median_per_attempt': statistics.median(values)} if values and all(value is not None for value in values) else None
        arms[arm] = {
            'planned': order.count(arm), 'attempted': len(attempted), 'accepted': accepted,
            'failed_or_rejected': len(attempted) - accepted,
            'conditional_api_equivalent_usd_all_attempts': total_cost,
            'conditional_api_equivalent_usd_per_accepted_task': total_cost / accepted if total_cost is not None and accepted else None,
            'actual_billed_usd_per_accepted_task': None,
            'metrics_all_attempts': {key: metric(key) for key in (
                'model_requests', 'raw_input_tokens', 'cached_input_tokens', 'cache_write_input_tokens',
                'legacy_net_input_tokens', 'output_tokens', 'final_authored_tokens_proxy', 'agent_wall_seconds', 'acceptance_wall_seconds')},
        }
    return {'complete': len(rows) == len(order), 'arms': arms,
            'planned_attempts': len(order), 'recorded_attempts': len(rows),
            'unlaunched_order': order[len(rows):], 'trials': rows}


def recount(path: Path) -> dict:
    data = json.loads(path.read_text())
    rows = []
    for original in data['trials']:
        label = f"{original['arm']}-{original['number']:02d}"
        stream, trace = Path(original['transcript']), Path(original['rollout_trace'])
        observed = campaign.trace_usage(campaign.parse_exec_jsonl(stream), trace)
        require(observed['reconciled'], f'{label}: raw usage does not reconcile')
        for key in ('model_request_count', 'request_usage_sum', 'legacy_net_input_tokens', 'model_observed', 'effort_observed'):
            require(observed[key] == original['observed'][key], f'{label}: saved {key} differs from trace')
        price = campaign.list_price_estimate(observed['model_requests'])
        require(price == original['list_price'], f'{label}: saved price differs from conditional recount')
        archive = Path(original['candidate_archive'])
        for relative, expected in original['candidate_files_sha256'].items():
            target = archive / relative
            require(target.is_file() and digest(target) == expected, f'{label}: archive hash differs: {relative}')
        metrics = original['final_candidate_source_metrics']
        for file in metrics['files']:
            require(digest(archive / file['path']) == file['sha256'], f'{label}: source metrics hash differs')
        require(sum(file['tokens'] for file in metrics['files']) == metrics['total_tokens'], f'{label}: source token sum differs')
        if original['status'] == 'accepted':
            require(original['acceptance']['accepted'], f'{label}: accepted without acceptance evidence')
            for key in ('workspace_integrity_before_acceptance', 'workspace_integrity_before_archive', 'workspace_integrity_before_cleanup'):
                require(original[key]['status'] == 'passed', f'{label}: accepted with failed integrity')
        usage = observed['request_usage_sum']
        rows.append({
            'arm': original['arm'], 'number': original['number'], 'status': original['status'],
            'failure': original['failure'], 'model_requests': observed['model_request_count'],
            'raw_input_tokens': usage['input_tokens'], 'cached_input_tokens': usage['cached_input_tokens'],
            'cache_write_input_tokens': usage['cache_write_input_tokens'], 'output_tokens': usage['output_tokens'],
            'reasoning_output_tokens_subset': usage['reasoning_output_tokens'],
            'legacy_net_input_tokens': observed['legacy_net_input_tokens'],
            'final_authored_tokens_proxy': metrics['total_tokens'],
            'agent_wall_seconds': original['elapsed_seconds'],
            'acceptance_wall_seconds': sum(original.get('acceptance', {}).get(key, {}).get('seconds', 0) for key in ('build', 'candidate_tests'))
                + sum(check.get('seconds', 0) for check in original.get('acceptance', {}).get('checks', [])),
            'conditional_api_equivalent_usd': price['standard_short_context_api_equivalent_usd'],
            'evidence_sha256': {'exec': digest(stream), 'rollout': digest(trace)},
        })
    result = summarize(data, rows)
    result.update({
        'schema': 'semaprax.codex-matched-recount.v1', 'results_sha256': digest(path),
        'provenance': {key: data['campaign'].get(key) for key in ('adapter', 'round', 'repository_commit', 'compiler_source_commit',
            'source_binary_sha256', 'seed_files_sha256', 'model_requested', 'effort_requested', 'codex_version', 'price_book', 'authored_source_tokenizer')},
        'measurement_notes': {
            'turns': 'Model requests from reconciled task-owned per-request rollout usage; outer CLI turns and tool calls are separate.',
            'input': 'Raw input includes cache subsets. Cache reads/writes are not added again.',
            'net_input': 'Historical proxy: sum input minus first-request input times model-request count. Not task-only input.',
            'authored': 'Final-source legacy Claude BPE proxy, including tests/docs/scripts. Not cumulative edits or exact GPT tokenization.',
            'cost': 'Conditional Standard short-context API-equivalent estimate, all attempted tasks divided by accepted tasks. Actual billed cost unavailable.',
            'model_identity': 'Client turn_context confirms requested model/effort. Provider-resolved model and tier unavailable.',
        },
        'fixed_harness_context_tokens': None,
        'fixed_harness_context_note': 'No per-request system/tool/task/history composition is exposed. Calibration is separate and is never subtracted.',
        'calibration_separate': data['calibration'],
    })
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', type=Path)
    args = parser.parse_args()
    print(json.dumps(recount(args.results.resolve(strict=True)), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
