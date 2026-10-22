#!/usr/bin/env python3
"""Run one committed focused OPT selection and production Clippy."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SELECTIONS = {
    'historical': ('ci-opt-batch-selection.json', 50, 153, ('lib', 'hir_module_boundaries')),
    'next': ('ci-opt-next-batch-selection.json', 17, 49, ('lib', 'owned_data')),
}
OUT = ROOT / 'diagnostic-opt-batch'
TARGET = ROOT / 'target/hosted-opt-batch'
PROFILE = {
    'CARGO_BUILD_JOBS': '1',
    'CARGO_INCREMENTAL': '0',
    **{f'CARGO_PROFILE_{kind}_{key}': value
       for kind in ('DEV', 'TEST')
       for key, value in {
           'DEBUG': '0', 'OPT_LEVEL': '0', 'CODEGEN_UNITS': '64',
           'DEBUG_ASSERTIONS': 'true', 'OVERFLOW_CHECKS': 'true',
           'INCREMENTAL': 'false', 'LTO': 'false',
       }.items()},
}
SUMMARY = re.compile(
    r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; '
    r'(\d+) measured; (\d+) filtered out'
)


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def text(args: list[str]) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def state(expected: str) -> dict[str, str]:
    head = text(['git', 'rev-parse', 'HEAD'])
    if head != expected:
        raise RuntimeError('checkout HEAD differs from the dispatched commit')
    subprocess.run(['git', 'diff', '--quiet', '--no-ext-diff', 'HEAD', '--'],
                   cwd=ROOT, check=True)
    tree = text(['git', 'rev-parse', 'HEAD^{tree}'])
    if text(['git', 'write-tree']) != tree:
        raise RuntimeError('index differs from the dispatched tree')
    return {'head': head, 'tree': tree}



def memory_sample(group: int) -> dict:
    # Observation only: no process RSS ceiling or available-memory stop policy.
    available = None
    for line in Path('/proc/meminfo').read_text().splitlines():
        if line.startswith('MemAvailable:'):
            available = int(line.split()[1]) * 1024
    rss = 0
    members = []
    page = os.sysconf('SC_PAGE_SIZE')
    for entry in Path('/proc').iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            raw = (entry / 'stat').read_text()
            fields = raw[raw.rfind(')') + 2:].split()
            if int(fields[2]) == group:
                members.append(int(entry.name))
                rss += max(0, int(fields[21])) * page
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    return {'unix_ns': time.time_ns(), 'mem_available_bytes': available,
            'command_group_rss_bytes': rss, 'member_pids': sorted(members)}


def settle(process: subprocess.Popen) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        pass
    finally:
        # Reap the direct child and stop any descendant that ignored SIGTERM,
        # including when the direct child exited before its own descendants.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--selection', choices=tuple(SELECTIONS), default='historical')
    options = parser.parse_args()
    filename, expected_filters, expected_tests, image_names = SELECTIONS[options.selection]
    selection = ROOT / 'scripts' / filename
    OUT.mkdir(exist_ok=False)
    receipt: dict = {
        'schema': 'semaprax.hosted-opt-batch.v1', 'status': 'running',
        'repository': os.environ.get('GITHUB_REPOSITORY'),
        'run_id': os.environ.get('GITHUB_RUN_ID'),
        'run_attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
        'qualification': 'focused OPT batch only; not the full quality profile',
        'commands': [], 'tests': [], 'binaries': {},
    }
    env = os.environ.copy()
    env.update(PROFILE)
    env['CARGO_TARGET_DIR'] = str(TARGET)
    tmp = TARGET / 'opt-batch-tmp'
    tmp.mkdir(parents=True)
    env['TMPDIR'] = str(tmp)

    def run(label: str, args: list[str], timeout: int, *,
            allow_failure: bool = False, sample_memory: bool = False) -> tuple[Path, dict]:
        log = OUT / f'{label}.log'
        item = {'label': label, 'argv': args, 'log': log.name}
        receipt['commands'].append(item)
        started = time.monotonic()
        pressure_path = OUT / f'{label}.memory.jsonl'
        pressure = pressure_path.open('w') if sample_memory else None
        samples = 0
        lowest_available = None
        highest_rss = None
        try:
            print(f'Starting {label}', flush=True)
            with log.open('wb') as stream:
                process = subprocess.Popen(args, cwd=ROOT, env=env, stdout=stream,
                                           stderr=subprocess.STDOUT, start_new_session=True)
                while True:
                    if pressure is not None:
                        try:
                            sample = memory_sample(process.pid)
                            sample['elapsed_seconds'] = time.monotonic() - started
                            available = sample['mem_available_bytes']
                            if available is not None:
                                lowest_available = available if lowest_available is None else min(lowest_available, available)
                            rss = sample['command_group_rss_bytes']
                            highest_rss = rss if highest_rss is None else max(highest_rss, rss)
                            samples += 1
                        except (OSError, ValueError, IndexError) as error:
                            sample = {'unix_ns': time.time_ns(), 'sampling_error': str(error)}
                        pressure.write(json.dumps(sample) + '\n')
                        pressure.flush()
                    remaining = timeout - (time.monotonic() - started)
                    if remaining <= 0:
                        item['timed_out'] = True
                        settle(process)
                        code = process.returncode
                        break
                    try:
                        code = process.wait(timeout=min(1.0, remaining))
                        break
                    except subprocess.TimeoutExpired:
                        continue
            item['exit_code'] = code
            if (code or item.get('timed_out')) and not allow_failure:
                raise RuntimeError(f'{label} exited {code}; timeout={item.get("timed_out", False)}; see {log.name}')
        finally:
            item['wall_seconds'] = time.monotonic() - started
            item['log_sha256'] = sha(log)
            if pressure is not None:
                pressure.close()
                item['memory'] = {
                    'log': pressure_path.name, 'sha256': sha(pressure_path), 'samples': samples,
                    'min_sampled_mem_available_bytes': lowest_available,
                    'max_sampled_command_group_rss_bytes': highest_rss,
                    'target_interval_seconds': 1.0,
                    'scope': 'system MemAvailable and summed member RSS (shared pages may repeat); not compiler phase or agent token usage',
                    'rss_cap_applied': False,
                }
        return log, item

    try:
        for name in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'):
            if env.get(name):
                raise RuntimeError(f'unexpected compiler flags: {name}')
        expected = os.environ['GITHUB_SHA']
        receipt.update(state(expected))
        receipt['profile_environment'] = {**PROFILE, 'CARGO_TARGET_DIR': str(TARGET)}
        receipt['runner_sha256'] = sha(Path(__file__))
        receipt['selection'] = {'profile': options.selection, 'path': str(selection.relative_to(ROOT)),
                                'expected_filters': expected_filters, 'expected_tests': expected_tests}
        receipt['selection_sha256'] = sha(selection)
        receipt['rustc'] = text(['rustc', '-vV'])
        receipt['cargo'] = text(['cargo', '--version'])
        receipt['native_tools'] = {name: env[name] for name in ('CLANG', 'SEMAPRAX_ARCHIVER')}
        receipt['history_inputs'] = 'Selected historical-body owners read committed fixtures, not historical Git objects; shallow checkout retained.'
        if not receipt['rustc'].startswith('rustc 1.97.1 '):
            raise RuntimeError('the owning toolchain must remain Rust 1.97.1')
        selected = json.loads(selection.read_text())
        rows = selected['commands']
        if (selected.get('schema') != 'semaprax.hosted-opt-batch-selection.v1'
                or selected.get('filters') != expected_filters
                or selected.get('expected_tests') != expected_tests
                or len(rows) != expected_filters
                or sum(r['expected_tests'] for r in rows) != expected_tests
                or {r['binary'] for r in rows} != set(image_names)):
            raise RuntimeError('selection differs from its committed filter/test/image profile')
        if options.selection == 'historical' and sum(
                r['binary'] == 'hir_module_boundaries' for r in rows) != 1:
            raise RuntimeError('historical boundary selector must remain unique')
        for row in rows:
            if (not isinstance(row['filter'], str) or not row['filter'].strip()
                    or row['filter'].startswith('-') or '\n' in row['filter']
                    or type(row['exact']) is not bool
                    or type(row['expected_tests']) is not int or row['expected_tests'] <= 0):
                raise RuntimeError('empty or invalid test selector')
        run('fetch', ['cargo', 'fetch', '--locked'], 900)
        artifacts: dict[str, list[Path]] = {name: [] for name in image_names}
        image_groups = [image_names] if options.selection == 'historical' else [
            (name,) for name in image_names]
        for group in image_groups:
            # Keep the historical build command; next builds each owning harness
            # once and serially with the same private target.
            image_args = []
            for image in group:
                image_args.extend(['--lib'] if image == 'lib' else ['--test', image])
            label = 'build' if options.selection == 'historical' else f'build-{group[0]}'
            build, _ = run(label, [
                'cargo', 'test', '--locked', '--offline', '-p', 'semaprax',
                *image_args, '--no-run', '--jobs', '1',
                '--message-format=json-render-diagnostics',
            ], 7200, sample_memory=True)
            finished = []
            with build.open() as stream:
                for line in stream:
                    try:
                        message = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if message.get('reason') == 'build-finished':
                        finished.append(message.get('success'))
                    target = message.get('target', {})
                    executable = message.get('executable')
                    if (message.get('reason') != 'compiler-artifact' or not executable
                            or not message.get('profile', {}).get('test')):
                        continue
                    for image in group:
                        if (image == 'lib' and target.get('name') == 'semaprax'
                                and target.get('kind') == ['lib']):
                            artifacts[image].append(Path(executable))
                        elif (image != 'lib' and target.get('name') == image
                              and target.get('kind') == ['test']):
                            artifacts[image].append(Path(executable))
            if finished != [True] or any(len(artifacts[image]) != 1 for image in group):
                raise RuntimeError('build did not emit exactly the selected owning test binaries')
        binaries = {}
        for kind, paths in artifacts.items():
            binary = paths[0]
            if (binary.is_symlink() or not binary.is_file()
                    or not binary.resolve().is_relative_to((TARGET / 'debug/deps').resolve())
                    or binary.stat().st_size <= 0):
                raise RuntimeError('unexpected test executable identity')
            binaries[kind] = binary.resolve()
            receipt['binaries'][kind] = {
                'path': str(binary.relative_to(ROOT)), 'bytes': binary.stat().st_size,
                'sha256': sha(binary),
            }
        def unchanged_binary(kind: str) -> None:
            binary = binaries[kind]
            if binary.is_symlink() or sha(binary) != receipt['binaries'][kind]['sha256']:
                raise RuntimeError(f'{kind} executable drift; aborting remaining execution')

        for index, row in enumerate(rows):
            state(expected)
            unchanged_binary(row['binary'])
            binary = binaries[row['binary']]
            record = {**row, 'index': index, 'status': 'discovery-pending',
                      'binary_sha256': receipt['binaries'][row['binary']]['sha256']}
            receipt['tests'].append(record)
            prefix = [str(binary), row['filter']]
            if row['exact']:
                prefix.append('--exact')
            listed, discovery = run(f'{index:02d}-list', prefix + ['--list', '--format', 'terse'],
                                    120, allow_failure=True)
            state(expected)
            unchanged_binary(row['binary'])
            names = [line.removesuffix(': test').strip()
                     for line in listed.read_text(errors='replace').splitlines() if line.endswith(': test')]
            record.update(listed_tests=names, discovered_count=len(names), discovery=discovery)
            if (discovery.get('exit_code') != 0 or discovery.get('timed_out')
                    or len(names) != row['expected_tests'] or not names):
                record['status'] = 'discovery-failed'
                record['error'] = 'failed discovery or exact nonzero count mismatch; filter not executed'
                record['completed'] = True
                continue
            record['status'] = 'failed'
            log, command = run(f'{index:02d}-test', prefix + ['--nocapture', '--test-threads=1'],
                               900, allow_failure=True)
            state(expected)
            unchanged_binary(row['binary'])
            record.update(log=log.name, exit_code=command['exit_code'],
                          timed_out=command.get('timed_out', False))
            summaries = []
            with log.open(errors='replace') as stream:
                for line in stream:
                    match = SUMMARY.search(line)
                    if match:
                        summaries.append({'result': match[1], **dict(zip(
                            ('passed', 'failed', 'ignored', 'measured', 'filtered_out'),
                            map(int, match.groups()[1:])))})
            record['summaries'] = summaries
            if len(summaries) == 1:
                summary = summaries[0]
                record.update(summary)
                if (command['exit_code'] == 0 and not command.get('timed_out')
                        and summary['result'] == 'ok'
                        and (summary['passed'], summary['failed'], summary['ignored'], summary['measured'])
                        == (row['expected_tests'], 0, 0, 0)):
                    record['status'] = 'passed'
            if record['status'] != 'passed':
                record['error'] = 'nonzero exit, timeout, missing/ambiguous summary, or nonpassing exact count'
            record['completed'] = True
        receipt['completed_filters'] = len(receipt['tests'])
        receipt['passed_filters'] = sum(row['status'] == 'passed' for row in receipt['tests'])
        receipt['failed_filters'] = sum(row['status'] != 'passed' for row in receipt['tests'])
        # Observed partial test counts are retained even when a filter fails.
        # A missing/ambiguous summary is unknown, never invented zero passes.
        receipt['observed_passed_tests'] = sum(row.get('passed', 0) for row in receipt['tests'])
        receipt['observed_failed_tests'] = sum(row.get('failed', 0) for row in receipt['tests'])
        receipt['filters_without_unique_summary'] = [row['index'] for row in receipt['tests']
                                                     if len(row.get('summaries', [])) != 1]
        state(expected)
        for kind in binaries:
            unchanged_binary(kind)
        _, clippy = run('clippy-production', [
            'cargo', 'clippy', '--locked', '--offline', '-p', 'semaprax',
            '--lib', '--bin', 'semaprax', '--jobs', '1', '--', '-D', 'warnings',
        ], 7200, allow_failure=True, sample_memory=True)
        receipt['clippy'] = {'scope': 'production lib + semaprax binary; warnings denied',
                            'passed': clippy['exit_code'] == 0 and not clippy.get('timed_out')}
        receipt['final_checkout'] = state(expected)
        for kind in binaries:
            unchanged_binary(kind)
        if (receipt['completed_filters'] != expected_filters or receipt['failed_filters'] != 0
                or receipt['observed_passed_tests'] != expected_tests or not receipt['clippy']['passed']):
            receipt['status'] = 'failed'
            receipt['error'] = 'one or more focused filters or production Clippy failed; see all collected outcomes'
            return 1
        receipt['passed_tests'] = expected_tests
        receipt['status'] = 'passed'
        return 0
    except Exception as error:
        receipt.update(status='failed', execution_aborted=True, error=f'{type(error).__name__}: {error}')
        print(receipt['error'], file=sys.stderr)
        return 1
    finally:
        receipt['attempted_filters'] = len(receipt['tests'])
        receipt['completed_filters'] = sum(row.get('completed', False) for row in receipt['tests'])
        receipt['passed_filters'] = sum(row['status'] == 'passed' for row in receipt['tests'])
        receipt['failed_filters'] = sum(row['status'] != 'passed' for row in receipt['tests'])
        receipt['observed_passed_tests'] = sum(row.get('passed', 0) for row in receipt['tests'])
        receipt['observed_failed_tests'] = sum(row.get('failed', 0) for row in receipt['tests'])
        receipt['filters_without_unique_summary'] = [row['index'] for row in receipt['tests']
                                                     if len(row.get('summaries', [])) != 1]
        receipt['summary_counts_complete'] = (receipt['completed_filters'] == expected_filters and not receipt['filters_without_unique_summary'])
        (OUT / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
        print(f'OPT batch status: {receipt["status"]}; receipts/logs: {OUT.name}', flush=True)


if __name__ == '__main__':
    raise SystemExit(main())
