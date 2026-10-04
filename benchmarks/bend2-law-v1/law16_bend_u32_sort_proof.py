#!/usr/bin/env python3
"""Capture/review the supplemental pinned Bend all-finite-U32-list source proof."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / 'benchmarks/bend2-law-v1'
FIXTURES = BENCH / 'fixtures/full-u32-encoding-v1'
PIN = '947db722640c86247849343657bf2f7ef01cb7f1'
SCHEMA = 'semaprax.bend2-law-benchmark.bend-u32-sort-proof.v1'
PASS = b'ALL PROOFS CHECK\n'
FAIL = b'SOME PROOFS FAIL\n'
TOOLS = ('main.ts', 'bend.ts', 'comp.ts', 'safe.ts', 'base.bend', 'bendtt.lean')


def digest(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def reference(path, root):
    data = path.read_bytes()
    return {'path': os.path.relpath(path, root), 'bytes': len(data), 'sha256': digest(data)}


def checked_reference(root, ref):
    path = root / ref['path']
    if path.is_symlink() or reference(path, root) != ref:
        raise ValueError('source or raw artifact drift: ' + str(path))
    return path


def negative_sources(source, proof):
    # Drop concrete controls, so only the universal multiplicity proof can fail.
    subject = source[:source.index('law case_0:')]
    old = '      insert(head, sort(tail))'
    if subject.count(old) != 1:
        raise ValueError('empty-sort mutation no longer uniquely matches')
    subject = subject.replace(old, '      Nil{}')
    count = proof[proof.index('# Natural counts'):]
    prefix = 'import Base\nimport ./empty-sort.bend as Sort\n\n'
    rejection = prefix + count
    # A proved 0-versus-1 witness distinguishes falsehood from a failed proof search.
    witness = prefix + count[:count.index('def count_step_swap')]
    witness += '''law empty_sort_loses_one:
  ({count(1, Sort.sort([1])) == 0n : Nat} & {count(1, [1]) == 1n : Nat})

def empty_sort_loses_one():
  ({==}, {==})
'''
    return {'empty-sort.bend': subject, 'empty-proof.bend': rejection,
            'empty-witness.bend': witness}


def run(argv, name, output, env):
    got = subprocess.run(argv, capture_output=True, env=env, timeout=120, check=False)
    for suffix, data in [('stdout', got.stdout), ('stderr', got.stderr)]:
        (output / f'{name}.{suffix}').write_bytes(data)
    return {'argv': argv, 'exit_code': got.returncode,
            'stdout': reference(output / f'{name}.stdout', output),
            'stderr': reference(output / f'{name}.stderr', output)}


def capture(bend, bun, output):
    if output.exists():
        raise ValueError('capture output directory must be new')
    bend, bun, output = bend.resolve(), bun.resolve(), output.resolve()
    head = subprocess.check_output(['git', '-C', str(bend), 'rev-parse', 'HEAD'], text=True).strip()
    if head != PIN:
        raise ValueError('Bend commit differs from the pinned comparison tool')
    for name in TOOLS:
        actual = (bend / 'bend2' / name).read_bytes()
        committed = subprocess.check_output(['git', '-C', str(bend), 'show', f'{PIN}:bend2/{name}'])
        if actual != committed:
            raise ValueError('Bend tool source differs from its pinned commit: ' + name)
    kernel_source = (bend / 'bend2/bendtt.lean').read_bytes()
    kernel = Path.home() / '.bend/bendtt' / hashlib.sha256(kernel_source).hexdigest()[:16] / 'bendtt'
    if not kernel.is_file():
        raise ValueError('provision the pinned BendTT kernel before capture')
    output.mkdir(parents=True)
    source = FIXTURES / 'sort.bend'
    proof = FIXTURES / 'sort-universal.bend'
    before = {p: p.read_bytes() for p in [source, proof, bun, kernel] + [bend / 'bend2' / name for name in TOOLS]}
    for name, text in negative_sources(source.read_text(), proof.read_text()).items():
        (output / name).write_text(text)
    env = dict(os.environ, BENDTT=str(kernel), LEAN_STACK_SIZE_KB='4194304')
    env.pop('BUN_OPTIONS', None)
    env.pop('NODE_OPTIONS', None)
    cli = [str(bun), str(bend / 'bend2/main.ts')]
    commands = {}
    commands['candidate'] = run(cli + [str(proof), '--verdict'], 'candidate', output, env)
    commands['empty_universal'] = run(cli + [str(output / 'empty-proof.bend'), '--verdict'], 'empty-universal', output, env)
    commands['empty_counterexample'] = run(cli + [str(output / 'empty-witness.bend'), '--verdict'], 'empty-counterexample', output, env)
    commands['export'] = run(cli + [str(proof), '-o', str(output / 'candidate.bendtt')], 'export', output, env)
    commands['kernel_replay'] = run([str(kernel), str(output / 'candidate.bendtt')], 'kernel-replay', output, env)
    if any(p.read_bytes() != content for p, content in before.items()):
        raise ValueError('source or tool changed during the physical proof run')
    capsule = {
        'schema': SCHEMA, 'status': 'supplemental_bend_u32_sort_source_proved',
        'bend_commit': PIN, 'source': reference(source, ROOT), 'proof': reference(proof, ROOT),
        'runner': reference(Path(__file__).resolve(), ROOT),
        'tools': {name: reference(bend / 'bend2' / name, bend) for name in TOOLS},
        'bun': {'path': str(bun), 'bytes': bun.stat().st_size, 'sha256': digest(bun.read_bytes())},
        'kernel': {'path': str(kernel), 'bytes': kernel.stat().st_size, 'sha256': digest(kernel.read_bytes())},
        'elaboration': reference(output / 'candidate.bendtt', output),
        'negative_sources': {name: reference(output / name, output) for name in negative_sources(source.read_text(), proof.read_text())},
        'commands': commands,
        'coverage': {'subject': 'Sort.sort from unchanged fixtures/full-u32-encoding-v1/sort.bend',
                     'domain': 'every finite List<U32>, every queried U32',
                     'sortedness': 'sorted_sort(xs): SortedB(Sort.sort(xs), 0)',
                     'multiplicity': 'count_sort(probe,xs): count(probe,Sort.sort(xs)) == count(probe,xs)',
                     'count_domain': 'unbounded Nat',
                     'permutation': 'extensional finite-multiset equality; see BEND-U32-SORT-PROOF.md'},
        'nonclaims': ['no SEMAPRAX law16 source certificate or paired timing',
                      'original v1 manifest admission unchanged',
                      'kernel binary/source association is local, not a reproducible build attestation',
                      'no runtime lowering, performance, or platform claim'],
    }
    (output / 'capsule.json').write_text(json.dumps(capsule, indent=2, sort_keys=True) + '\n')
    verify(output / 'capsule.json')
    return capsule


def verify(path):
    capsule = json.loads(path.read_text())
    if capsule['schema'] != SCHEMA or capsule['bend_commit'] != PIN:
        raise ValueError('capsule schema or pinned tool identity differs')
    if capsule['status'] != 'supplemental_bend_u32_sort_source_proved':
        raise ValueError('unsupported proof disposition')
    for field, expected_path in [('source', FIXTURES / 'sort.bend'), ('proof', FIXTURES / 'sort-universal.bend'), ('runner', Path(__file__).resolve())]:
        if checked_reference(ROOT, capsule[field]).resolve() != expected_path.resolve():
            raise ValueError('capsule binds an unrelated proof input')
    folder = path.parent
    checked_reference(folder, capsule['elaboration'])
    expected = negative_sources((ROOT / capsule['source']['path']).read_text(), (ROOT / capsule['proof']['path']).read_text())
    if set(capsule['negative_sources']) != set(expected):
        raise ValueError('missing negative source')
    for name, ref in capsule['negative_sources'].items():
        if checked_reference(folder, ref).read_text() != expected[name]:
            raise ValueError('negative source does not derive from exact candidate')
    names = {'candidate', 'empty_universal', 'empty_counterexample', 'export', 'kernel_replay'}
    if set(capsule['commands']) != names:
        raise ValueError('missing physical command evidence')
    expected_input = {'candidate': capsule['proof']['path'], 'empty_universal': 'empty-proof.bend', 'empty_counterexample': 'empty-witness.bend'}
    for name, cell in capsule['commands'].items():
        stdout = checked_reference(folder, cell['stdout']).read_bytes()
        stderr = checked_reference(folder, cell['stderr']).read_bytes()
        if name == 'empty_universal':
            if cell['exit_code'] != 1 or FAIL not in stdout + stderr or b'Location: count_sort' not in stdout + stderr:
                raise ValueError('empty sort lacks exact universal count-law rejection')
        elif cell['exit_code'] != 0 or (name != 'export' and stdout != PASS):
            raise ValueError('proof or kernel replay did not pass: ' + name)
        if name in expected_input:
            if len(cell['argv']) != 4 or cell['argv'][-1] != '--verdict' or not cell['argv'][-2].endswith('/' + expected_input[name]):
                raise ValueError('missing exact proof input and actual verdict command')
            if cell['argv'][0] != capsule['bun']['path'] or not cell['argv'][1].endswith('/bend2/main.ts'):
                raise ValueError('verdict tool identity differs')
        if name == 'kernel_replay' and (len(cell['argv']) != 2 or cell['argv'][0] != capsule['kernel']['path'] or not cell['argv'][1].endswith('/candidate.bendtt')):
            raise ValueError('kernel replay identity differs')
    return capsule


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bend-root', type=Path)
    parser.add_argument('--bun', type=Path)
    parser.add_argument('--output-dir', type=Path)
    parser.add_argument('--verify', type=Path)
    args = parser.parse_args()
    if args.verify:
        verify(args.verify)
    elif args.bend_root and args.bun and args.output_dir:
        capture(args.bend_root, args.bun, args.output_dir)
    else:
        parser.error('provide --verify, or --bend-root, --bun and --output-dir')
    print('Bend all-finite-U32-list source proof capsule verified')


if __name__ == '__main__':
    main()
