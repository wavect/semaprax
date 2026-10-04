#!/usr/bin/env python3
"""Capture paired cold/warm proof verdicts in the pinned Linux guest."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import uuid

ROOT = Path(__file__).resolve().parent
IMAGE = 'ri13-linux-evidence:rust-1.98.0'
PIN_SCHEMA = 'semaprax.bend2-law-benchmark.native-arm64-z3-source-obligation-pins.v1'
SHA256 = re.compile(r'sha256:[0-9a-f]{64}')
IMAGE_SHA = 'sha256:c9024b5897124ae3a7f124a41dbe4d301c7319daf4e9aed82b23424648eb311e'
BEND_COMMIT = '947db722640c86247849343657bf2f7ef01cb7f1'
SEMAPRAX_COMMIT = 'cdfc0cdd27aa248951134f70a71bf7d36b4798de'
TOOL_PINS = {
    'bun': 'sha256:c356f5fb6f75a0a83d918ee2391bc4ec1e6061baa08b0ee1c3a5cfb4aaf3d567',
    'semaprax': 'sha256:5d2ecf0a63ce86c967e3f2f1a2f509c71cdfbb4aacddc32e856a17bb1e8d7b97',
    'z3': 'sha256:23ccbf81c4375cb1925ace3f082c4998396736e525887cb5847c8b581e88fcaf',
    'bendtt': 'sha256:72e11a86f44563e9decb26fe1c586ed7d5465e98ca282ada5ae8753b1156cad5',
}
Z3_LIBRARY_PINS = {
    'libz3.so': 'sha256:1cfad33e1579b194cce477c55c3b6bd287f07285f54c6130a48f8986a01ae46a',
    'libz3java.so': 'sha256:607bfefb13f1914cf849790037307245d4d472e67ff43199e4740ed413bb40ee',
}
NONCLAIMS = [
    'guest file-page residency covers directly inventoried route files; host, hardware, solver-internal and translation caches are unknown',
    'the ARM64 route runs Z3 on retained source-derived SMT-LIB; it makes no SEMAPRAX ARM compilation or project-proof-check claim',
    'the direct Z3 result is limited to the pinned source-derived obligation; no model, lowering, execution, or SEMAPRAX project-proof-check claim follows',
    'thirty paired cold/warm samples describe each independently pinned profile only',
    'no cross-language winner, timing ratio, public support claim, or issue closure follows from this capture',
]

def digest(path):
    return 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()

def digest_bytes(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()

def ref(path, root):
    return {'path': path.relative_to(root).as_posix(), 'bytes': path.stat().st_size, 'sha256': digest(path)}

def verify_python_image(image_doc, architecture):
    expected_arch = {'amd64': 'amd64', 'arm64': 'arm64'}[architecture]
    variants = image_doc[0].get('variants', []) if image_doc else []
    for variant in variants:
        config = variant.get('config', {})
        platform_info = variant.get('platform', {})
        actual_arch = platform_info.get('architecture', config.get('architecture'))
        if actual_arch != expected_arch:
            continue
        history = config.get('history', [])
        commands = ' '.join(row.get('created_by', '') for row in history if isinstance(row, dict))
        if 'python3' in commands:
            return
    raise ValueError(f'pinned {architecture} guest image does not document a Python 3 runtime')

def load_worker():
    spec = importlib.util.spec_from_file_location('proof_cache_worker', ROOT / 'law16_proof_verdict_cache_worker.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def safe_ref(value, label):
    if not isinstance(value, dict) or set(value) != {'path', 'bytes', 'sha256'}:
        raise ValueError(f'{label} pin shape differs')
    path = Path(value['path'])
    if path.is_absolute() or not path.parts or '..' in path.parts or not isinstance(value['bytes'], int) or value['bytes'] <= 0 or not isinstance(value['sha256'], str) or not SHA256.fullmatch(value['sha256']):
        raise ValueError(f'{label} pin is malformed')
    return value

def read_arm64_pins(path, expected_sha256):
    raw = path.read_bytes()
    if not SHA256.fullmatch(expected_sha256) or digest(path) != expected_sha256:
        raise ValueError('native arm64 pins digest differs from independently approved SHA-256')
    value = json.loads(raw)
    canonical = (json.dumps(value, separators=(',', ':'), sort_keys=True) + '\n').encode()
    if raw != canonical:
        raise ValueError('native arm64 pins must use canonical JSON')
    if (not isinstance(value, dict) or set(value) != {'schema', 'profile', 'tools', 'z3_libraries', 'image', 'image_sha256', 'obligation', 'source_script', 'source', 'source_commit', 'renderer_version'}
            or value.get('schema') != PIN_SCHEMA or value.get('profile') != 'native_arm64_z3_source_obligation' or not isinstance(value.get('image'), str)
            or '@sha256:' not in value.get('image', '') or not SHA256.fullmatch('sha256:' + value['image'].rsplit('@sha256:', 1)[-1])
            or not isinstance(value.get('image_sha256'), str) or not SHA256.fullmatch(value['image_sha256'])):
        raise ValueError('native arm64 pins schema or profile differs')
    tools, libraries = value.get('tools'), value.get('z3_libraries')
    if (not isinstance(tools, dict) or set(tools) != {'z3'}
            or not isinstance(libraries, dict)):
        raise ValueError('native arm64 Z3 pin inventory differs')
    if any(not isinstance(digest_value, str) or not SHA256.fullmatch(digest_value) for digest_value in [*tools.values(), *libraries.values()]):
        raise ValueError('native arm64 tool pins must be sha256 digests')
    if any(not isinstance(name, str) or Path(name).name != name or not name.startswith('libz3') for name in libraries):
        raise ValueError('native arm64 Z3 library names are unsafe')
    for label in ('obligation', 'source_script', 'source'):
        safe_ref(value.get(label), label)
    if Path(value['obligation']['path']).suffix != '.smt2' or value['source']['sha256'] != 'sha256:bc71b8bde8cb43cc10b49742e7e5abb3717cd28402a1ab53e7d549f540093063':
        raise ValueError('ARM64 obligation or fixed source identity differs')
    if value['source_commit'] != 'cdfc0cdd27aa248951134f70a71bf7d36b4798de' or not isinstance(value['renderer_version'], str) or not value['renderer_version'].strip():
        raise ValueError('source commit or renderer version pin differs')
    return value

def validate_source_obligation_bytes(pins, script, source, helper):
    if (digest_bytes(source) != pins['source']['sha256'] or digest_bytes(script) != pins['obligation']['sha256']
            or digest_bytes(helper) != pins['source_script']['sha256'] or not script.endswith(b'(check-sat)\n')
            or b'(get-model)' in script):
        raise ValueError('retained ARM source-derived SMT obligation differs from its pinned inputs')
    if pins.get('source_commit') != 'cdfc0cdd27aa248951134f70a71bf7d36b4798de':
        raise ValueError('source commit pin differs')
    return True

def checked_input(path, reference, label):
    path = path.resolve(strict=True)
    if path.is_symlink() or not path.is_file() or path.stat().st_size != reference['bytes'] or digest(path) != reference['sha256']:
        raise ValueError(f'{label} bytes differ from the independently approved manifest')
    return path

def copy_pinned_input(source, reference, destination, label):
    relative = Path(reference['path'])
    if relative.is_absolute() or '..' in relative.parts:
        raise ValueError(f'{label} path is unsafe')
    target = destination.joinpath(*relative.parts)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    if target.stat().st_size != reference['bytes'] or digest(target) != reference['sha256']:
        raise ValueError(f'{label} changed while staging')
    return target

def check_tools(paths):
    for name, path in paths.items():
        if not path.is_file() or path.is_symlink() or digest(path) != TOOL_PINS[name]:
            raise ValueError(f'{name} is missing or differs from the immutable tool pin')

def check_z3_libraries(z3):
    for name, expected in Z3_LIBRARY_PINS.items():
        path = z3.parent / name
        if not path.is_file() or path.is_symlink() or digest(path) != expected:
            raise ValueError(f'Z3 shared library is missing or differs from its immutable pin: {name}')

def check_bend(root):
    head = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
    dirty = subprocess.check_output(
        ['git', '-C', str(root), 'status', '--porcelain', '--untracked-files=all', '--', 'bend2'],
        text=True,
    )
    if head != BEND_COMMIT or dirty:
        raise ValueError('Bend source must be the clean pinned bend2 tree')
    return {p.relative_to(root / 'bend2').as_posix(): digest(p) for p in sorted((root / 'bend2').rglob('*')) if p.is_file() and not p.is_symlink()}

def validate(root, expected_pins):
    worker = load_worker()
    result_path = root / 'guest-result.json'
    result = json.loads(result_path.read_text())
    if result.get('schema') != 'semaprax.bend2-law-benchmark.proof-verdict-cache-guest.v2' or result.get('status') != 'thirty_cold_warm_pairs_for_profile_observed':
        raise ValueError('guest did not complete all thirty successful profile pairs')
    if result.get('scope') != worker.SCOPE or result.get('guest', {}).get('page_size') != 4096:
        raise ValueError('guest scope or page size differs')
    if result.get('source_pins') != expected_pins or result.get('tool_pins') != expected_pins['tools'] or result.get('z3_library_pins') != expected_pins['z3_libraries']:
        raise ValueError('guest profile pin receipt differs')
    worker.validate_result(result, root)
    route = 'bend_verdict' if expected_pins['profile'] == 'pinned_x86_rosetta_bend_verdict' else 'z3_source_obligation'
    if result.get('route_counts') != {route: {'pairs': 30, 'cold': 30, 'warm': 30}}:
        raise ValueError('profile route sample counts differ')
    return result

def review(root, expected_pins_sha256=None):
    root = root.resolve(strict=True)
    receipt = json.loads((root / 'receipt.json').read_text())
    if receipt.get('schema') != 'semaprax.bend2-law-benchmark.proof-verdict-cache-capture.v2' or receipt.get('status') != 'captured' or receipt.get('container_exit_code') != 0 or receipt.get('container_removed') is not True or receipt.get('nonclaims') != NONCLAIMS:
        raise ValueError('capture receipt, guest image, cleanup, or scope differs')
    pins = json.loads((root / 'source-pins.json').read_text())
    if pins != receipt.get('source_pins') or pins.get('profile') not in ('native_arm64_z3_source_obligation', 'pinned_x86_rosetta_bend_verdict'):
        raise ValueError('saved source pin receipt differs')
    if digest(root / 'source-pins.json') != receipt.get('pins_sha256'):
        raise ValueError('saved pin bytes differ from the captured pin digest')
    if pins['profile'] == 'native_arm64_z3_source_obligation':
        if expected_pins_sha256 is None or not SHA256.fullmatch(expected_pins_sha256) or expected_pins_sha256 != receipt.get('pins_sha256') or digest(root / 'source-pins.json') != expected_pins_sha256:
            raise ValueError('native arm64 pins require the independently approved expected SHA-256 during review')
        read_arm64_pins(root / 'source-pins.json', expected_pins_sha256)
    elif pins.get('tools') != {'bun': TOOL_PINS['bun'], 'bendtt': TOOL_PINS['bendtt']}:
        raise ValueError('pinned x86/Rosetta tool hashes differ')
    argv = receipt.get('container_argv')
    arch = 'arm64' if pins['profile'].startswith('native_arm64') else 'amd64'
    if not isinstance(argv, list) or argv.count('--arch') != 1:
        raise ValueError('guest architecture is missing from the capture command')
    arch_index = argv.index('--arch')
    if arch_index + 1 >= len(argv) or argv[arch_index + 1] != arch:
        raise ValueError('guest architecture does not match the pinned physical profile')
    if ('--rosetta' in argv) != (arch == 'amd64'):
        raise ValueError('Rosetta mode does not match the pinned physical profile')
    expected_image_digest = pins['image_sha256'] if pins['profile'] == 'native_arm64_z3_source_obligation' else IMAGE_SHA
    expected_image = pins['image'] if pins['profile'] == 'native_arm64_z3_source_obligation' else IMAGE
    if receipt.get('image_digest') != expected_image_digest or receipt.get('image') != expected_image:
        raise ValueError('receipt guest image does not match the pinned profile')
    worker_path = root / 'worker.py'
    if (not worker_path.is_file() or worker_path.is_symlink() or digest(worker_path) != receipt.get('worker_sha256')
            or digest(worker_path) != digest(ROOT / 'law16_proof_verdict_cache_worker.py')):
        raise ValueError('retained worker does not match the executed and reviewed worker')
    image = json.loads((root / 'image-inspect.json').read_text())
    if not image or image[0]['configuration']['descriptor']['digest'] != expected_image_digest:
        raise ValueError('saved guest image inspection differs')
    verify_python_image(image, 'arm64' if pins['profile'] == 'native_arm64_z3_source_obligation' else 'amd64')
    if pins['profile'] == 'native_arm64_z3_source_obligation':
        for label in ('obligation', 'source_script', 'source'):
            ref = pins[label]
            target = root.joinpath(*Path(ref['path']).parts)
            if target.is_symlink() or not target.is_file() or target.stat().st_size != ref['bytes'] or digest(target) != ref['sha256']:
                raise ValueError(f'retained {label} differs from the approved ARM pin manifest')
        source_bytes = (root.joinpath(*Path(pins['source']['path']).parts)).read_bytes()
        script_bytes = (root.joinpath(*Path(pins['obligation']['path']).parts)).read_bytes()
        helper_bytes = (root.joinpath(*Path(pins['source_script']['path']).parts)).read_bytes()
        validate_source_obligation_bytes(pins, script_bytes, source_bytes, helper_bytes)
    tool_pins = receipt.get('tool_pins')
    if not isinstance(tool_pins, dict) or set(tool_pins) != set(pins['tools']) or any(not isinstance(row, dict) or row.get('sha256') != pins['tools'][name] or not isinstance(row.get('path'), str) or not isinstance(row.get('bytes'), int) or row['bytes'] <= 0 for name, row in tool_pins.items()):
        raise ValueError('tool pin receipt differs')
    artifacts = receipt.get('artifacts')
    if not isinstance(artifacts, list):
        raise ValueError('capture artifact inventory is missing')
    artifact_paths = set()
    for item in artifacts:
        if not isinstance(item, dict) or set(item) != {'path', 'bytes', 'sha256'}:
            raise ValueError('capture artifact row is malformed')
        relative = Path(item.get('path', ''))
        if relative.is_absolute() or '..' in relative.parts or not relative.parts or not (root / relative).is_file() or (root / relative).is_symlink():
            raise ValueError('capture artifact path is unsafe or missing')
        path = root / relative
        if path.stat().st_size != item.get('bytes') or digest(path) != item.get('sha256'):
            raise ValueError('capture artifact identity drifted')
        if str(relative) in artifact_paths:
            raise ValueError('duplicate capture artifact reference')
        artifact_paths.add(str(relative))
    actual_paths = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file() and p.name != 'receipt.json'}
    if artifact_paths != actual_paths:
        raise ValueError('capture artifact inventory does not cover every retained output')
    result = validate(root, pins)
    return {'schema': receipt['schema'], 'status': 'thirty_cold_warm_pairs_per_profile_authenticated', 'profile': pins['profile'], 'samples': len(result['samples']), 'source_pins': receipt['source_pins'], 'nonclaims': NONCLAIMS}

def capture(args):
    args.output = args.output.resolve()
    if args.output.exists() or not args.output.parent.is_dir():
        raise ValueError('output must be new beneath an existing directory')
    if shutil.disk_usage(args.output.parent).free < 2 * 1024**3:
        raise ValueError('capture requires 2 GiB free for the bounded output and reserve')
    bend_files = None
    if args.profile == 'native_arm64_z3_source_obligation':
        args.z3 = args.z3.resolve(strict=True)
        args.arm64_pins = args.arm64_pins.resolve(strict=True)
        pins = read_arm64_pins(args.arm64_pins, args.expected_pins_sha256)
        pins_digest = args.expected_pins_sha256
        image_ref, image_sha = pins['image'], pins['image_sha256']
        inputs = {'obligation': args.obligation, 'source_script': args.source_script, 'source': ROOT / 'fixtures/boolean-negation-project-v1/candidate/src/app.spx'}
        checked = {name: checked_input(path, pins[name], name) for name, path in inputs.items()}
        tools_in = {'z3': args.z3}
        arch_flags = ['--arch', 'arm64']
    else:
        args.bun = args.bun.resolve(strict=True)
        args.bendtt = args.bendtt.resolve(strict=True)
        args.bend = args.bend.resolve(strict=True)
        bend_files = check_bend(args.bend)
        tools_in = {'bun': args.bun, 'bendtt': args.bendtt}
        pins = {'schema': 'semaprax.bend2-law-benchmark.pinned-x86-rosetta-bend-tool-pins.v1',
                'profile': 'pinned_x86_rosetta_bend_verdict', 'tools': {name: TOOL_PINS[name] for name in tools_in},
                'z3_libraries': {}, 'bend_commit': BEND_COMMIT, 'bend2_files': bend_files}
        pins_digest = digest_bytes((json.dumps(pins, separators=(',', ':'), sort_keys=True) + '\n').encode())
        image_ref, image_sha = IMAGE, IMAGE_SHA
        arch_flags = ['--arch', 'amd64', '--rosetta']
    for name, path in tools_in.items():
        if path.is_symlink() or digest(path) != pins['tools'][name]:
            raise ValueError(f'{name} differs from the pinned profile tool digest')
    if args.profile.startswith('native_arm64'):
        for name, expected in pins['z3_libraries'].items():
            path = args.z3.parent / name
            if path.is_symlink() or not path.is_file() or digest(path) != expected:
                raise ValueError(f'{name} differs from the independently approved native arm64 pin')
    image_raw = subprocess.check_output(['container', 'image', 'inspect', image_ref])
    image_doc = json.loads(image_raw)
    if not image_doc or image_doc[0]['configuration']['descriptor']['digest'] != image_sha:
        raise ValueError('local immutable guest image pin differs')
    verify_python_image(image_doc, 'arm64' if args.profile.startswith('native_arm64') else 'amd64')
    args.output.mkdir()
    if args.profile.startswith('native_arm64'):
        checked = {name: copy_pinned_input(source, pins[name], args.output, name) for name, source in checked.items()}
    (args.output / 'image-inspect.json').write_bytes(image_raw)
    (args.output / 'source-pins.json').write_bytes((json.dumps(pins, separators=(',', ':'), sort_keys=True) + '\n').encode())
    mounts = args.output / 'mounts'
    tools = mounts / 'tools'; tools.mkdir(parents=True)
    for name, source in tools_in.items():
        target = tools / name
        shutil.copyfile(source, target)
        target.chmod(0o755)
    for name, expected in pins['z3_libraries'].items():
        source = args.z3.parent / name
        if digest(source) != expected:
            raise ValueError(f'Z3 shared library changed during capture setup: {name}')
        shutil.copyfile(source, tools / name)
    mounts_list = [(tools, '/tools', True)]
    if args.profile.startswith('native_arm64'):
        for name, destination in [('obligation', mounts / 'obligation.smt2'), ('source_script', mounts / 'source-script')]:
            shutil.copyfile(checked[name], destination)
        source_dir = mounts / 'source/src'; source_dir.mkdir(parents=True)
        shutil.copyfile(checked['source'], source_dir / 'app.spx')

    else:
        fixture = mounts / 'fixtures'; fixture.mkdir()
        shutil.copyfile(ROOT / 'fixtures/bend-boolean-negation-v1.bend', fixture / 'bend-boolean-negation-v1.bend')
        bend_mount = mounts / 'bend2'; shutil.copytree(args.bend / 'bend2', bend_mount, symlinks=True)
        mounts_list += [(fixture, '/fixtures', True), (bend_mount, '/bend2', True)]
    shutil.copyfile(ROOT / 'law16_proof_verdict_cache_worker.py', mounts / 'worker.py')
    (mounts / 'source-pins.json').write_bytes((json.dumps(pins, separators=(',', ':'), sort_keys=True) + '\n').encode())
    argv = ['container', 'run', '--rm', '--name', 'law16-proof-cache-' + uuid.uuid4().hex[:10], '--memory', '512M', '--cpus', '1'] + arch_flags + ['--read-only-path', 'NONE', '--no-dns']
    mounts_list += [(mounts, '/capture', True), (args.output, '/output', False)]
    for source, dest, readonly in mounts_list:
        argv += ['--mount', f'type=bind,source={source},target={dest}' + (',readonly' if readonly else '')]
    argv += [image_ref, 'python3', '/capture/worker.py']
    try:
        completed = subprocess.run(argv, capture_output=True, timeout=900)
        code, stdout, stderr = completed.returncode, completed.stdout, completed.stderr
    except subprocess.TimeoutExpired as error:
        code, stdout, stderr = None, error.stdout or b'', error.stderr or b''
    finally:
        if (mounts / 'worker.py').is_file():
            shutil.copyfile(mounts / 'worker.py', args.output / 'worker.py')
        subprocess.run(['container', 'stop', '--time', '2', argv[4]], capture_output=True, timeout=15)
        subprocess.run(['container', 'delete', argv[4]], capture_output=True, timeout=15)
        shutil.rmtree(mounts, ignore_errors=True)
    (args.output / 'container.stdout').write_bytes(stdout)
    (args.output / 'container.stderr').write_bytes(stderr)
    after = subprocess.check_output(['container', 'list', '--all', '--format', 'json'])
    (args.output / 'containers-after.json').write_bytes(after)
    if sum(p.stat().st_size for p in args.output.rglob('*') if p.is_file()) > 1024**3:
        raise ValueError('capture exceeded the 1 GiB artifact bound')
    removed = argv[4] not in after.decode()
    if code != 0 or not removed:
        raise ValueError('guest process failed or container cleanup was not observed')
    validate(args.output, pins)
    files = [p for p in sorted(args.output.rglob('*')) if p.is_file() and p.name != 'receipt.json']
    receipt = {'schema': 'semaprax.bend2-law-benchmark.proof-verdict-cache-capture.v2', 'status': 'captured', 'container_argv': argv, 'container_exit_code': code, 'container_removed': removed, 'image': image_ref, 'image_digest': image_sha, 'pins_sha256': pins_digest, 'source_pins': pins, 'worker_sha256': digest(args.output / 'worker.py'), 'tool_pins': {name: {'path': str(path), 'bytes': path.stat().st_size, 'sha256': digest(path)} for name, path in tools_in.items()}, 'host': {'platform': platform.platform(), 'architecture': platform.machine()}, 'nonclaims': NONCLAIMS, 'artifacts': [ref(path, args.output) for path in files]}
    (args.output / 'receipt.json').write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    return validate(args.output, pins)

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--review', type=Path)
    parser.add_argument('--expected-pins-sha256')
    parser.add_argument('--profile', choices=('native_arm64_z3_source_obligation', 'pinned_x86_rosetta_bend_verdict'))
    parser.add_argument('--z3', type=Path)
    parser.add_argument('--arm64-pins', type=Path)
    parser.add_argument('--obligation', type=Path)
    parser.add_argument('--source-script', type=Path)
    parser.add_argument('--bun', type=Path)
    parser.add_argument('--bendtt', type=Path)
    parser.add_argument('--bend', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    try:
        if args.review:
            print(json.dumps(review(args.review, args.expected_pins_sha256), indent=2, sort_keys=True))
            return 0
        if args.profile == 'native_arm64_z3_source_obligation':
            if not all((args.z3, args.arm64_pins, args.expected_pins_sha256, args.obligation, args.source_script, args.output)):
                parser.error('native ARM Z3 capture requires pinned Z3, pins, obligation, source renderer, expected pins SHA-256, and output')
        elif args.profile == 'pinned_x86_rosetta_bend_verdict':
            if not all((args.bun, args.bendtt, args.bend, args.output)):
                parser.error('x86/Rosetta capture requires --bun, --bendtt, --bend, and --output')
        else:
            parser.error('capture requires --profile')
        result = capture(args)
        print(json.dumps({'status': result['status'], 'samples': len(result['samples']), 'output': str(args.output)}, sort_keys=True))
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        parser.error(str(error))

if __name__ == '__main__':
    raise SystemExit(main())
