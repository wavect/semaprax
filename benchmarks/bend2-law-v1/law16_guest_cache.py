#!/usr/bin/env python3
"""Capture/review the explicit Linux/Rosetta guest file-page-cache profile."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parent
SCHEMA = 'semaprax.bend2-law-benchmark.guest-cache.v1'
IMAGE = 'ri13-linux-evidence:rust-1.98.0'
IMAGE_SHA = 'sha256:c9024b5897124ae3a7f124a41dbe4d301c7319daf4e9aed82b23424648eb311e'
BEND_COMMIT = '947db722640c86247849343657bf2f7ef01cb7f1'
TOOLS = {
    'semaprax': {'path': 'semaprax/semaprax-v0.7.0-x86_64-unknown-linux-gnu/semaprax', 'sha256': 'sha256:f9bf5d83bdde585eb4f0fc07570730a0bc07930b589ad9eaec4217205c188c95'},
    'bun': {'path': 'bun/bun-linux-x64-baseline/bun', 'sha256': 'sha256:c356f5fb6f75a0a83d918ee2391bc4ec1e6061baa08b0ee1c3a5cfb4aaf3d567'},
    'libc6': {'path': 'libc6_2.39-0ubuntu8.9_amd64.deb', 'sha256': 'sha256:ff5557d99b51f761c4b7c92368b9cc45565eda17df9bf9eb4b134d09825008be'},
}
VERSIONS = {'semaprax': 'semaprax 0.7.0 (eec951eb1cce83e5e0f42edf97cbb5b8f3cffa2c)', 'bun': '1.2.5'}
GUEST_SCOPE = 'Linux guest file page cache only; host and Rosetta caches unknown'
PREFIX = ['/opt/law16/glibc/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2', '--library-path', '/opt/law16/glibc/usr/lib/x86_64-linux-gnu']
COMMANDS = {'bend_ordinary': PREFIX + ['/opt/law16/bun', '/opt/law16/bend2/main.ts', '/opt/law16/candidate.bend'],
            'semaprax_check': PREFIX + ['/opt/law16/semaprax', 'check', '/opt/law16/candidate.spx']}
EXPECTED_STDOUT = {'bend_ordinary': b'True\nFalse\n',
                   'semaprax_check': b'verified /opt/law16/candidate.spx (sha256:0bdb415a6ef882b762d05c64133533be1a4f9ab67996abcc752f8909ff4a5032)\n'}
RUNTIME = {'glibc/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2': 'sha256:c20a2dc8917c755f02b94049356320fe1f62ac7d9f8994731f807d9df39302da',
           'glibc/usr/lib/x86_64-linux-gnu/libc.so.6': 'sha256:3a15d66867d83762c7f2f1e37359cb8f6c5743edb369c65285cb0b1c4f7498bf',
           'glibc/usr/lib/x86_64-linux-gnu/libm.so.6': 'sha256:fce00b6f25f459cf4ae0b7fae4257909a1a8a86f9149e7c029a5d617baf1ccd0'}
NONCLAIMS = ['guest file-page-cache scope only; macOS host cache and Rosetta translation cache are unknown',
             'live Python/system-library pages and hardware caches are outside the direct residency inventory',
             'ordinary Bend and SEMAPRAX check only; no verdict, SMT, Lean, or formal proof measurement',
             'new Linux/Rosetta release profile; not historical macOS executable pins',
             'no cross-language winner, ratio, native-hardware timing, or issue closure']


def sha(path):
    return 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()


def ref(path, root):
    return {'path': path.relative_to(root).as_posix(), 'bytes': path.stat().st_size, 'sha256': sha(path)}


def checked(root, value):
    path = root / value['path']
    if Path(value['path']).is_absolute() or '..' in Path(value['path']).parts or path.is_symlink() or not path.is_file() or ref(path, root) != value:
        raise ValueError('raw artifact identity drifted')
    return path


def review(root):
    receipt = json.loads((root / 'receipt.json').read_text())
    if receipt.get('schema') != SCHEMA or receipt.get('container_exit_code') != 0 or receipt.get('image_digest') != IMAGE_SHA:
        raise ValueError('guest capture schema, image, or exit status differs')
    for value in receipt['artifacts']:
        checked(root, value)
    request = json.loads((root / 'request.json').read_text())
    result = json.loads((root / 'guest-result.json').read_text())
    count = request.get('repetitions')
    if count not in (1, 30) or request.get('tools') != TOOLS or request.get('versions') != VERSIONS or request.get('bend_commit') != BEND_COMMIT:
        raise ValueError('guest request tool pins or repetition count differ')
    if result.get('status') != ('pilot_guest_cache_observed' if count == 1 else 'guest_cache_thirty_pairs_observed'):
        raise ValueError('guest cache observation did not complete')
    if result.get('scope') != GUEST_SCOPE or result['guest']['page_size'] != 4096 or result.get('environment') != {'BEND_NO_TELEMETRY': '1', 'BUN_RUNTIME_TRANSPILER_CACHE_PATH': '0', 'DO_NOT_TRACK': '1'}:
        raise ValueError('guest cache scope or environment differs')
    if receipt.get('container_removed') is not True or receipt.get('nonclaims') != NONCLAIMS:
        raise ValueError('guest cleanup or explicit cache boundary differs')
    for name, fixture in [('candidate.bend', 'bend-boolean-negation-v1.bend'), ('candidate.spx', 'semaprax-boolean-negation-v1.spx')]:
        path = checked(root / 'inputs', request['inputs'][name])
        if path.read_bytes() != (ROOT / 'fixtures' / fixture).read_bytes():
            raise ValueError('guest subject differs from the fixed Boolean pair')
    expected = [(i, lane) for i in range(1, count + 1) for lane in ('bend_ordinary', 'semaprax_check')]
    if [(row['ordinal'], row['lane']) for row in result['samples']] != expected:
        raise ValueError('guest sample inventory differs')
    times = {lane: {'cold': [], 'warm': []} for lane in ('bend_ordinary', 'semaprax_check')}
    for tool, version in VERSIONS.items():
        observed = result['versions'][tool]
        if observed['exit_code'] != 0 or observed['argv'] != PREFIX + ['/opt/law16/' + tool, '--version'] or checked(root, observed['stdout']).read_text().strip() != version:
            raise ValueError('observed guest tool version differs')
    for row in result['samples']:
        inventory = result['file_inventory'][row['lane']]
        tool, source = ('bun', 'candidate.bend') if row['lane'] == 'bend_ordinary' else ('semaprax', 'candidate.spx')
        required = {**RUNTIME, tool: TOOLS[tool]['sha256'], source: request['inputs'][source]['sha256']}
        if row['lane'] == 'bend_ordinary':
            required.update({'bend2/' + name: digest for name, digest in request['bend_files'].items()})
        if any(inventory.get(name, {}).get('sha256') != digest for name, digest in required.items()):
            raise ValueError('residency inventory lacks pinned tool, source, or runtime')
        for field in ('before_reset', 'before_cold', 'before_warm'):
            if set(row[field]) != set(inventory):
                raise ValueError('residency inventory differs')
        for name, cold in row['before_cold'].items():
            before = row['before_reset'][name]
            if cold['resident_pages'] != 0 or before['resident_pages'] != before['pages'] or cold['pages'] != before['pages'] or cold['pages'] != (inventory[name]['bytes'] + 4095) // 4096:
                raise ValueError('file pages were not observed resident then guest-cache cold')
        if not all(row['before_warm'][name]['resident_pages'] > 0 for name in (tool, source)):
            raise ValueError('warm residency not observed')
        for state in ('cold', 'warm'):
            sample = row[state]
            if sample['exit_code'] != 0 or sample['timed_out'] or sample['elapsed_ns'] <= 0 or sample['argv'] != COMMANDS[row['lane']]:
                raise ValueError('timed command failed or differs from admitted preflight')
            if checked(root, sample['stderr']).read_bytes() or checked(root, sample['stdout']).read_bytes() != EXPECTED_STDOUT[row['lane']]:
                raise ValueError('checker output differs from the admitted Boolean result')
            times[row['lane']][state].append(sample['elapsed_ns'])
        if row['cold']['stdout']['sha256'] != row['warm']['stdout']['sha256']:
            raise ValueError('cold/warm output differs')
    summary = {lane: {state: {'count': len(values), 'p50_ns': statistics.median(values), 'p95_ns': sorted(values)[max(0, (95 * len(values) + 99) // 100 - 1)], 'mad_ns': statistics.median(abs(value - statistics.median(values)) for value in values)} for state, values in states.items()} for lane, states in times.items()}
    return {'schema': SCHEMA, 'status': 'pilot_authenticated' if count == 1 else 'thirty_guest_cache_pairs_authenticated', 'summary': summary, 'nonclaims': NONCLAIMS}


def capture(args):
    if args.output.exists() or not args.output.parent.is_dir() or shutil.disk_usage(args.output.parent).free < 2 * 1024**3:
        raise ValueError('output must be new with at least 2 GiB free')
    if args.repetitions == 30 and (not args.pilot or review(args.pilot)['status'] != 'pilot_authenticated'):
        raise ValueError('thirty pairs require a previously authenticated one-pair pilot')
    for row in TOOLS.values():
        if sha(args.tools / row['path']) != row['sha256']:
            raise ValueError('provisioned tool digest differs')
    head = subprocess.check_output(['git', '-C', str(args.bend), 'rev-parse', 'HEAD'], text=True).strip()
    if head != BEND_COMMIT or subprocess.run(['git', '-C', str(args.bend), 'diff', '--quiet', 'HEAD', '--', 'bend2']).returncode:
        raise ValueError('Bend source is not the clean pinned tree')
    image = subprocess.check_output(['container', 'image', 'inspect', IMAGE])
    if json.loads(image)[0]['configuration']['descriptor']['digest'] != IMAGE_SHA:
        raise ValueError('local guest image pin differs')
    args.output.mkdir()
    inputs = args.output / 'inputs'; inputs.mkdir()
    for name, fixture in [('candidate.bend', 'bend-boolean-negation-v1.bend'), ('candidate.spx', 'semaprax-boolean-negation-v1.spx')]:
        shutil.copyfile(ROOT / 'fixtures' / fixture, inputs / name)
    shutil.copyfile(ROOT / 'law16_guest_cache_worker.py', inputs / 'worker.py')
    bend_files = [name.removeprefix('bend2/') for name in subprocess.check_output(['git', '-C', str(args.bend), 'ls-tree', '-r', '--name-only', 'HEAD', '--', 'bend2'], text=True).splitlines()]
    request = {'repetitions': args.repetitions, 'tools': TOOLS, 'versions': VERSIONS, 'bend_commit': BEND_COMMIT,
               'bun_source_commit': '013fdddc6ed18bc849614ccb37a296fa0f69a5db',
               'inputs': {name: ref(inputs / name, inputs) for name in ('candidate.bend', 'candidate.spx')},
               'bend_files': {name: sha(args.bend / 'bend2' / name) for name in bend_files}}
    (args.output / 'request.json').write_text(json.dumps(request, indent=2, sort_keys=True) + '\n')
    (args.output / 'image-inspect.json').write_bytes(image)
    name = 'law16-guest-cache-' + uuid.uuid4().hex[:10]
    argv = ['container', 'run', '--rm', '--name', name, '--memory', '512M', '--cpus', '1', '--arch', 'amd64', '--rosetta', '--read-only-path', 'NONE', '--no-dns']
    for source, dest, readonly in [(args.tools, '/tools', True), (args.bend / 'bend2', '/bend2', True), (inputs, '/inputs', True), (args.output, '/output', False)]:
        argv += ['--mount', f'type=bind,source={source},target={dest}' + (',readonly' if readonly else '')]
    argv += [IMAGE, 'python3', '/inputs/worker.py']
    try:
        completed = subprocess.run(argv, capture_output=True, timeout=900)
        code, stdout, stderr = completed.returncode, completed.stdout, completed.stderr
    except subprocess.TimeoutExpired as error:
        code, stdout, stderr = None, error.stdout or b'', error.stderr or b''
    finally:
        subprocess.run(['container', 'stop', '--time', '2', name], capture_output=True, timeout=15)
        subprocess.run(['container', 'delete', name], capture_output=True, timeout=15)
    (args.output / 'container.stdout').write_bytes(stdout)
    (args.output / 'container.stderr').write_bytes(stderr)
    after = subprocess.check_output(['container', 'list', '--all', '--format', 'json'])
    (args.output / 'containers-after.json').write_bytes(after)
    receipt = {'schema': SCHEMA, 'container_argv': argv, 'container_exit_code': code, 'container_removed': name not in after.decode(),
               'image_digest': IMAGE_SHA, 'host': {'platform': platform.platform(), 'architecture': platform.machine()},
               'nonclaims': NONCLAIMS, 'artifacts': [ref(path, args.output) for path in sorted(args.output.rglob('*')) if path.is_file()]}
    (args.output / 'receipt.json').write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    return review(args.output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--review', type=Path)
    parser.add_argument('--tools', type=Path)
    parser.add_argument('--bend', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--repetitions', type=int, choices=(1, 30), default=1)
    parser.add_argument('--pilot', type=Path)
    args = parser.parse_args()
    try:
        if args.review:
            result = review(args.review.resolve())
        else:
            if not all((args.tools, args.bend, args.output)):
                parser.error('capture requires --tools, --bend, and --output')
            args.tools, args.bend, args.output = args.tools.resolve(), args.bend.resolve(), args.output.resolve()
            result = capture(args)
        print(json.dumps(result, indent=2, sort_keys=True))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
