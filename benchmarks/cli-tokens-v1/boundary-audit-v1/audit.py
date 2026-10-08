#!/usr/bin/env python3
"""Independent, post-round LogLens boundary qualification on candidate copies.

No live harness/oracle import, model invocation, source repair or archive writes.
Expected output comes from authored request facts, not parsing through a second
copy of the candidate's parser. Historical acceptance and saved costs remain
separate from this additive runtime qualification.
"""
from __future__ import annotations
import argparse
from collections import Counter
from decimal import Decimal
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time

HERE = Path(__file__).resolve().parent
SPEC = HERE.parent / 'SPEC.md'
CORPUS = HERE / 'corpus.json'
SCHEMA = 'semaprax.loglens-boundary-audit.v1'


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def request_line(row: dict) -> bytes:
    """Only generate the unambiguous frozen request shape used by these probes."""
    assert re.fullmatch(r'\S+', row['ip'])
    assert row['path'].startswith('/') and not re.search(r'[\s"\\]', row['path'])
    assert re.fullmatch(r'[0-9]{2}', row['hour']) and '00' <= row['hour'] <= '23'
    assert 100 <= row['status'] <= 599
    assert row['bytes'] == '-' or re.fullmatch(r'[0-9]+', row['bytes'])
    zone = row.get('zone', '+0000')
    assert re.fullmatch(r'\+[0-9]{4}', zone)
    return (f"{row['ip']} - - [10/Oct/2026:{row['hour']}:00:00 {zone}] "
            f'"GET {row["path"]} HTTP/1.1" {row["status"]} {row["bytes"]}').encode('utf-8')


def materialize(case: dict) -> tuple[bytes, list[dict], int]:
    if case.get('recipe') == 'exactly-65536-bytes':
        row = case['request']
        line = request_line(row)
        count = (65536 - len(line)) // (len(line) + 1)
        prefix = (line + b'\n') * count
        padding = b'\n' * (65536 - len(prefix) - len(line))
        data = prefix + padding + line
        assert len(data) == 65536 and not data.endswith(b'\n')
        return data, [row] * (count + 1), count + 1
    data = bytearray()
    requests, nonempty = [], 0
    rows = case['rows']
    separators = case.get('terminators', ['\n'])
    for index, row in enumerate(rows):
        if 'request' in row:
            requests.append(row['request'])
            line = request_line(row['request'])
        elif 'malformed' in row:
            line = row['malformed'].encode('utf-8')
            assert not any(char in line for char in (10, 13))
        else:
            assert row == {'empty': True}
            line = b''
        nonempty += bool(line)
        data.extend(line)
        if index < len(rows) - 1 or case.get('final_terminated', True):
            data.extend(separators[index % len(separators)].encode('ascii'))
    assert len(data) <= 65536
    return bytes(data), requests, nonempty


def expected(case: dict) -> tuple[bytes, bytes]:
    _, rows, nonempty = materialize(case)
    categories = Counter(f"{row['status'] // 100}xx" for row in rows if row['status'] >= 200)
    paths = Counter(row['path'] for row in rows)
    hours = Counter(row['hour'] for row in rows)
    total = sum(0 if row['bytes'] == '-' else int(row['bytes']) for row in rows)
    count = len(rows)
    errors = categories['4xx'] + categories['5xx']
    # Integer quotient/remainder half-up, independent of float or Decimal context.
    tenths = 0 if count == 0 else (errors * 2000 + count) // (2 * count)
    rate = f'{tenths // 10}.{tenths % 10}'
    ranked = sorted(paths.items(), key=lambda pair: (-pair[1], pair[0].encode('utf-8')))[:case['top']]
    ordered_hours = sorted(hours.items())
    busiest = min(hours, key=lambda hour: (-hours[hour], hour)) if hours else '-'
    status = {key: categories[key] for key in ('2xx', '3xx', '4xx', '5xx')}
    report = {'lines': nonempty, 'requests': count, 'malformed': nonempty - count,
              'unique_ips': len({row['ip'] for row in rows}), 'status': status, 'error_rate': rate,
              'bytes': total, 'avg_bytes': total // count if count else 0,
              'top_paths': [{'path': path, 'count': n} for path, n in ranked],
              'hours': dict(ordered_hours), 'busiest_hour': busiest}
    encoded = json.dumps(report, ensure_ascii=False, separators=(',', ':'))
    encoded = encoded.replace(f'"error_rate":"{rate}"', f'"error_rate":{rate}', 1)
    text = [f'lines: {nonempty}', f'requests: {count}', f'malformed: {nonempty - count}',
            f'unique_ips: {report["unique_ips"]}',
            'status: ' + ' '.join(f'{key}={value}' for key, value in status.items()),
            f'error_rate: {rate}%', f'bytes: {total}', f'avg_bytes: {report["avg_bytes"]}', 'top_paths:']
    text.extend(f'  {index}. {path} {n}' for index, (path, n) in enumerate(ranked, 1))
    text.append('hours:')
    text.extend(f'  {hour} {n}' for hour, n in ordered_hours)
    text.append(f'busiest_hour: {busiest}')
    return ('\n'.join(text) + '\n').encode('utf-8'), (encoded + '\n').encode('utf-8')


def inventory(directory: Path) -> dict[str, str]:
    return {path.relative_to(directory).as_posix(): sha(path.read_bytes())
            for path in sorted(directory.rglob('*')) if path.is_file()}


def execute(command: list[str], candidate: Path, env: dict, logs: Path, label: str, timeout: int) -> dict:
    started = time.monotonic()
    out, err = logs / f'{label}.stdout', logs / f'{label}.stderr'
    with out.open('wb') as stdout, err.open('wb') as stderr:
        process = subprocess.Popen(command, cwd=candidate, env=env, stdout=stdout, stderr=stderr,
                                   start_new_session=True)
        expired = False
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            expired = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                code = process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                code = process.wait(timeout=2)
    return {'command': command, 'exit_code': code, 'timed_out': expired,
            'elapsed_seconds': round(time.monotonic() - started, 6),
            'stdout_path': str(out), 'stderr_path': str(err),
            'stdout_sha256': sha(out.read_bytes()), 'stderr_sha256': sha(err.read_bytes()),
            'stdout_bytes': out.stat().st_size, 'stderr_bytes': err.stat().st_size}


def execution_mode(candidate: Path, arm: str) -> dict:
    script = (candidate / 'run.sh').read_text()
    calls = [line.strip() for line in script.splitlines() if not line.lstrip().startswith('#')
             and (re.search(r'\bexec\b', line) or re.match(r'\s*node\s', line) or re.search(r'\srun\s', line))]
    native = []
    for path in sorted(candidate.rglob('*')):
        if not path.is_file() or path.suffix in ('.log', '.json', '.txt'):
            continue
        with path.open('rb') as handle:
            magic = handle.read(4)
        if magic in (b'\xcf\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xce\xfa\xed\xfe', b'\x7fELF'):
            native.append(path.relative_to(candidate).as_posix())
    if arm == 'semaprax' and any(re.search(r'\brun\s', line) for line in calls):
        mode = 'interpreter_via_semaprax_run'
    elif arm == 'semaprax' and native:
        mode = 'native_executable_via_wrapper'
    elif arm == 'typescript' and any(re.search(r'\bnode\b', line) for line in calls):
        mode = 'node_via_wrapper'
    else:
        mode = 'wrapper_unclassified'
    return {'mode': mode, 'basis': 'copied entry wrapper and built executable magic; no native-only acceptance requirement',
            'entry_calls': calls, 'native_artifacts': native,
            'run_script_sha256': sha((candidate / 'run.sh').read_bytes())}


def summarize(rows: list[dict], planned: int, historical: list[dict] | None = None) -> dict:
    paid = historical if historical is not None else rows
    result = {'planned_paid_attempts': planned, 'recorded_paid_attempts': len(paid), 'audited_attempts': len(rows), 'arms': {}}
    for arm in ('semaprax', 'typescript'):
        selected = [row for row in rows if row['arm'] == arm]
        attempted = [row for row in paid if row['arm'] == arm]
        qualified = sum(row['expanded_qualification_passed'] for row in selected)
        values = [row['saved_conditional_api_equivalent_usd'] if 'saved_conditional_api_equivalent_usd' in row
                  else row.get('list_price', {}).get('standard_short_context_api_equivalent_usd') for row in attempted]
        total = sum((Decimal(str(value)) for value in values), Decimal(0)) if values and all(value is not None for value in values) else None
        result['arms'][arm] = {'attempted': len(attempted), 'audited': len(selected), 'not_yet_audited': len(attempted) - len(selected),
            'historical_33_check_accepted': sum(row.get('historical_status', row.get('status')) == 'accepted' for row in attempted),
            'expanded_qualification_accepted': qualified,
            'failed_expanded_qualification': len(selected) - qualified,
            'saved_conditional_api_equivalent_usd_all_attempts': str(total) if total is not None else None,
            'conditional_api_equivalent_usd_per_expanded_accepted_task': str(total / qualified) if total is not None and qualified else None,
            'actual_billing_receipt_usd': None}
    return result


def audit(results: Path, output: Path, build_timeout: int, case_timeout: int) -> dict:
    source_bytes = results.read_bytes()
    data = json.loads(source_bytes)
    planned = data['campaign']['trial_order']
    trials = data['trials']
    if len(planned) != 10 or len(trials) != len(planned):
        raise ValueError('audit requires the completed matched 5/arm round; no live candidate execution')
    counts = {'semaprax': 0, 'typescript': 0}
    for trial, arm in zip(trials, planned):
        counts[arm] += 1
        if (trial['arm'], trial['number']) != (arm, counts[arm]):
            raise ValueError('trial order/identity does not match the frozen plan')
    if counts != {'semaprax': 5, 'typescript': 5}:
        raise ValueError('completed matched round must contain five attempts per arm')
    corpus_bytes = CORPUS.read_bytes()
    corpus = json.loads(corpus_bytes)
    if sha(SPEC.read_bytes()) != corpus['spec_sha256']:
        raise ValueError('audit SPEC changed from the pinned frozen contract')
    if data['campaign']['seed_files_sha256'].get('benchmarks/cli-tokens-v1/SPEC.md') != corpus['spec_sha256']:
        raise ValueError('campaign SPEC differs from the independent audit contract')
    compiler = Path(data['campaign']['semaprax_binary']).resolve(strict=True)
    if sha(compiler.read_bytes()) != data['campaign']['source_binary_sha256']:
        raise ValueError('frozen campaign compiler binary hash changed')
    output = output.resolve()
    if output.exists():
        raise ValueError('audit output must be new')
    campaign_root = results.resolve().parent
    if output == campaign_root or campaign_root in output.parents:
        raise ValueError('audit output must be outside historical campaign artifacts')
    output.mkdir(parents=True)
    (output / 'corpus.json').write_bytes(corpus_bytes)
    fixtures = output / 'fixtures'
    fixtures.mkdir()
    for case in corpus['cases']:
        content, _, _ = materialize(case)
        (fixtures / f'{case["name"]}.log').write_bytes(content)
        text, encoded = expected(case)
        (fixtures / f'{case["name"]}.expected.txt').write_bytes(text)
        (fixtures / f'{case["name"]}.expected.json').write_bytes(encoded)
    env = os.environ.copy()
    env.update({'SEMAPRAX_BIN': str(compiler), 'npm_config_offline': 'true',
                'PATH': str(compiler.parent) + os.pathsep + env.get('PATH', '')})
    rows = []
    for trial in trials:
        label = f'{trial["arm"]}-{trial["number"]:02d}'
        archive = Path(trial['candidate_archive']).resolve(strict=True)
        before = inventory(archive)
        if before != trial['candidate_files_sha256']:
            raise ValueError(f'{label}: archived source inventory differs from frozen manifest')
        candidate = output / 'copies' / label
        candidate.parent.mkdir(exist_ok=True)
        shutil.copytree(archive, candidate)
        logs = output / 'logs' / label
        logs.mkdir(parents=True)
        row = {'arm': trial['arm'], 'number': trial['number'], 'archive': str(archive),
               'archive_files_sha256': before, 'copy': str(candidate),
               'historical_status': trial['status'], 'historical_acceptance_unchanged': trial.get('acceptance'),
               'saved_conditional_api_equivalent_usd': trial.get('list_price', {}).get('standard_short_context_api_equivalent_usd'),
               'boundary_checks': [], 'expanded_qualification_passed': False}
        try:
            row['build'] = execute(['/bin/sh', str(candidate / 'build.sh')], candidate, env, logs, 'build', build_timeout)
            row['execution'] = execution_mode(candidate, trial['arm'])
            if row['build']['exit_code'] == 0 and not row['build']['timed_out']:
                for case in corpus['cases']:
                    input_file = fixtures / f'{case["name"]}.log'
                    for form in ('text', 'json'):
                        arguments = [str(input_file), '--top', str(case['top'])] + (['--json'] if form == 'json' else [])
                        check = execute(['/bin/sh', str(candidate / 'run.sh'), *arguments], candidate, env, logs,
                                        f'{case["name"]}-{form}', case_timeout)
                        expected_file = fixtures / f'{case["name"]}.expected.{"txt" if form == "text" else "json"}'
                        wanted = expected_file.read_bytes()
                        check.update({'case': case['name'], 'form': form, 'input_sha256': sha(input_file.read_bytes()),
                                      'input_bytes': input_file.stat().st_size, 'expected_stdout_sha256': sha(wanted),
                                      'status': 'timeout' if check['timed_out'] else 'passed' if check['exit_code'] == 0
                                        and Path(check['stdout_path']).read_bytes() == wanted else 'failed'})
                        row['boundary_checks'].append(check)
            row['expanded_qualification_passed'] = (trial['status'] == 'accepted'
                and trial.get('acceptance', {}).get('accepted') is True
                and len(row['boundary_checks']) == 2 * len(corpus['cases'])
                and all(check['status'] == 'passed' for check in row['boundary_checks']))
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            row['audit_failure'] = str(error)
        row['historical_archive_unchanged'] = inventory(archive) == before
        if not row['historical_archive_unchanged']:
            raise ValueError(f'{label}: historical archive changed during audit; evidence invalid')
        rows.append(row)
        report = {'schema': SCHEMA, 'historical_results': str(results.resolve()), 'historical_results_sha256': sha(source_bytes),
                  'spec_sha256': corpus['spec_sha256'], 'corpus_sha256': sha(corpus_bytes),
                  'compiler_sha256': data['campaign']['source_binary_sha256'],
                  'compiler_source_commit': data['campaign']['compiler_source_commit'],
                  'build_timeout_seconds': build_timeout, 'case_timeout_seconds': case_timeout,
                  'criterion': 'saved historical 33-check acceptance AND independent boundary text+JSON byte equality',
                  'paid_attempt_cost_scope': 'all saved attempts, including boundary failures; calibration separate; estimate is not receipt',
                  'audit_complete': len(rows) == len(trials), 'summary_provisional': len(rows) != len(trials), 'summary': summarize(rows, len(planned), trials), 'trials': rows}
        (output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
        print(f'{label}: {sum(c["status"] == "passed" for c in row["boundary_checks"])}/{2 * len(corpus["cases"])} boundary checks; expanded={row["expanded_qualification_passed"]}', flush=True)
    if results.read_bytes() != source_bytes:
        raise ValueError('historical results changed during post-round audit')
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--build-timeout', type=int, default=180)
    parser.add_argument('--case-timeout', type=int, default=120)
    args = parser.parse_args()
    if not 1 <= args.build_timeout <= 600 or not 1 <= args.case_timeout <= 300:
        parser.error('build timeout must be 1..600 and per-case timeout 1..300 seconds')
    try:
        audit(args.results, args.output, args.build_timeout, args.case_timeout)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(2, f'audit error: {error}\n')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
