#!/usr/bin/env python3
"""Run one profile-specific paired cold/warm proof route inside a Linux guest."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

WORK = Path('/opt/law16')
OUT = Path('/output')
PINS = {'bun': 'sha256:c356f5fb6f75a0a83d918ee2391bc4ec1e6061baa08b0ee1c3a5cfb4aaf3d567',
        'semaprax': 'sha256:5d2ecf0a63ce86c967e3f2f1a2f509c71cdfbb4aacddc32e856a17bb1e8d7b97',
        'z3': 'sha256:23ccbf81c4375cb1925ace3f082c4998396736e525887cb5847c8b581e88fcaf',
        'bendtt': 'sha256:72e11a86f44563e9decb26fe1c586ed7d5465e98ca282ada5ae8753b1156cad5'}
Z3_LIBRARY_PINS = {'libz3.so': 'sha256:1cfad33e1579b194cce477c55c3b6bd287f07285f54c6130a48f8986a01ae46a',
                   'libz3java.so': 'sha256:607bfefb13f1914cf849790037307245d4d472e67ff43199e4740ed413bb40ee'}
SOURCES = {'fixtures/bend-boolean-negation-v1.bend': 'sha256:91eeeb8c2a9a055a38022e00bf02381a4788cc8608c1d96148dad17e5f109b81',
           'project/src/app.spx': 'sha256:bc71b8bde8cb43cc10b49742e7e5abb3717cd28402a1ab53e7d549f540093063',
           'project/semaprax.toml': 'sha256:481999d6499263a605ce8eaf67e8155b772f6cddbe466c87e3087978ff339bb5',
           'project/core/core.spx': 'sha256:79ca3e75303ff52c305458af8c9e81b1ff52da7c7fc0bb994b86e8e017d273a5',
           'project/tests/tests.spx': 'sha256:fed58ed7a469fcfd90b3a0c59ef27b44b71d15529cbd522df8088dfdde29e443'}
SCOPE = 'Linux guest file-page residency for directly inventoried route files; host, hardware, solver-internal and translation caches are unknown'
SEMAPRAX_COMMIT = 'cdfc0cdd27aa248951134f70a71bf7d36b4798de'
ROUTES = {
    'bend_verdict': ['/tools/bun', '/bend2/main.ts', '/fixtures/bend-boolean-negation-v1.bend', '--verdict'],
    'semaprax_z3_project_proof_check': ['/tools/semaprax', 'project-proof-check', '/project/semaprax.toml', '--tool', 'z3', '--executable', '/tools/z3', '--version-line', 'Z3 version 4.12.5 - 64 bit', '--host-profile', 'trusted-local', '--source', 'src/app.spx', '--declaration', 'app.negate', '--ensures', '0'],
}
PROFILES = {
    'pinned_x86_rosetta_bend_verdict': {'route': 'bend_verdict', 'machine': ('x86_64',), 'tools': ('bun', 'bendtt')},
    'native_arm64_semaprax_z3_project_proof_check': {'route': 'semaprax_z3_project_proof_check', 'machine': ('aarch64', 'arm64'), 'tools': ('semaprax', 'z3')},
}

def sha(path):
    return 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()

def resident(path):
    size = path.stat().st_size
    pages = (size + os.sysconf('SC_PAGE_SIZE') - 1) // os.sysconf('SC_PAGE_SIZE')
    if size == 0:
        return {'pages': 0, 'resident_pages': 0}
    libc = ctypes.CDLL(None, use_errno=True)
    libc.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_long]
    libc.mmap.restype = ctypes.c_void_p
    libc.mincore.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p]
    libc.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    with path.open('rb') as stream:
        address = libc.mmap(None, size, 0, 2, stream.fileno(), 0)
        if address == ctypes.c_void_p(-1).value:
            raise OSError(ctypes.get_errno(), 'mmap failed')
        try:
            vector = (ctypes.c_ubyte * pages)()
            if libc.mincore(address, size, vector):
                raise OSError(ctypes.get_errno(), 'mincore failed')
            return {'pages': pages, 'resident_pages': sum(x & 1 for x in vector)}
        finally:
            libc.munmap(address, size)

def run(route, state, ordinal, argv):
    started = time.monotonic_ns()
    try:
        child = subprocess.run(argv, capture_output=True, timeout=120, env=dict(os.environ, BENDTT='/tools/bendtt', BEND_NO_TELEMETRY='1', BUN_RUNTIME_TRANSPILER_CACHE_PATH='0', DO_NOT_TRACK='1', LD_LIBRARY_PATH='/tools', HOME='/tmp/law16-home'))
        code, stdout, stderr, expired = child.returncode, child.stdout, child.stderr, False
    except subprocess.TimeoutExpired as error:
        code, stdout, stderr, expired = None, error.stdout or b'', error.stderr or b'', True
    row = {'argv': argv, 'exit_code': code, 'timed_out': expired, 'elapsed_ns': time.monotonic_ns() - started}
    for channel, data in [('stdout', stdout), ('stderr', stderr)]:
        path = OUT / f'{route}-{ordinal:02d}-{state}.{channel}'
        path.write_bytes(data)
        row[channel] = {'path': path.name, 'bytes': len(data), 'sha256': sha(path)}
    return row

def validate_result(result, output):
    """Fail closed if a route, success stream, or raw byte reference is absent."""
    route = result.get('route')
    profile = result.get('profile')
    if profile not in PROFILES or route != PROFILES[profile]['route']:
        raise ValueError('profile and fixed proof route differ')
    expected = [(route, ordinal) for ordinal in range(1, 31)]
    observed = [(row.get('route'), row.get('ordinal')) for row in result.get('samples', [])]
    if observed != expected or result.get('scope') != SCOPE:
        raise ValueError('capture does not contain thirty ordered pairs for both fixed proof routes')
    tool_pins = result.get('tool_pins')
    library_pins = result.get('z3_library_pins')
    if not isinstance(tool_pins, dict) or set(tool_pins) != set(PROFILES[profile]['tools']):
        raise ValueError('profile tool pins are incomplete')
    if profile.startswith('native_arm64') and not isinstance(library_pins, dict):
        raise ValueError('native ARM Z3 library pin map is malformed')
    pinned_rows = [('/tools/' + name, digest, route) for name, digest in tool_pins.items()]
    pinned_rows += [('/tools/' + name, digest, route) for name, digest in library_pins.items()]
    source_prefix = 'project/' if route.startswith('semaprax') else 'fixtures/'
    pinned_rows += [('/' + name, digest, route) for name, digest in SOURCES.items() if name.startswith(source_prefix)]
    if route == 'bend_verdict':
        pinned_rows += [('/bend2/' + name, digest, route) for name, digest in result.get('source_pins', {}).get('bend2_files', {}).items()]
    for name, expected_digest, route in pinned_rows:
        if result.get('file_inventory', {}).get(route, {}).get(name, {}).get('sha256') != expected_digest:
            raise ValueError('pinned tool, library, or source digest is absent from residency inventory')
    preflight = result.get('preflight', {}).get(route)
    if not isinstance(preflight, dict) or preflight.get('exit_code') != 0 or preflight.get('timed_out'):
        raise ValueError('successful proof preflight is missing')
    for channel in ('stdout', 'stderr'):
        ref = preflight.get(channel, {})
        path = output / ref.get('path', '')
        if (not isinstance(ref.get('path'), str) or Path(ref['path']).name != ref['path'] or not path.is_file()
                or path.is_symlink() or path.stat().st_size != ref.get('bytes') or sha(path) != ref.get('sha256')):
            raise ValueError('preflight raw proof stream is missing or drifted')
    preflight_stdout = (output / preflight['stdout']['path']).read_bytes()
    if (route == 'bend_verdict' and b'ALL PROOFS CHECK' not in preflight_stdout) or (route.startswith('semaprax') and b'app.negate' not in preflight_stdout):
        raise ValueError('preflight did not report the expected successful proof verdict')
    for sample in result['samples']:
        inventory = result.get('file_inventory', {}).get(sample['route'])
        if not isinstance(inventory, dict) or not inventory:
            raise ValueError('residency inventory is missing for a proof route')
        for field in ('before_reset', 'before_cold', 'before_warm'):
            if set(sample.get(field, {})) != set(inventory):
                raise ValueError('residency receipt does not cover the complete file inventory')
        for name, item in inventory.items():
            pages = (item['bytes'] + 4095) // 4096
            reset, cold, warm = sample['before_reset'][name], sample['before_cold'][name], sample['before_warm'][name]
            if (reset.get('pages') != pages or reset.get('resident_pages') != pages
                    or cold.get('pages') != pages or cold.get('resident_pages') != 0
                    or warm.get('pages') != pages or warm.get('resident_pages') != pages):
                raise ValueError('guest pages were not observed warm then fully cold')
        for state in ('cold', 'warm'):
            row = sample.get(state, {})
            if row.get('exit_code') != 0 or row.get('timed_out') or row.get('elapsed_ns', 0) <= 0:
                raise ValueError('proof route sample did not succeed')
            for channel in ('stdout', 'stderr'):
                ref = row.get(channel, {})
                relative = ref.get('path', '')
                if not isinstance(relative, str) or Path(relative).name != relative:
                    raise ValueError('raw proof stream reference is unsafe')
                path = output / relative
                if not path.is_file() or path.is_symlink() or path.stat().st_size != ref.get('bytes') or sha(path) != ref.get('sha256'):
                    raise ValueError('raw proof stream is missing or drifted')
            stdout = (output / row['stdout']['path']).read_bytes()
            if not stdout or (sample['route'] == 'bend_verdict' and b'ALL PROOFS CHECK' not in stdout) or (sample['route'].startswith('semaprax') and b'app.negate' not in stdout):
                raise ValueError('successful proof verdict is not present in raw stdout')
    return True

def main():
    pins = json.loads(Path('/source-pins.json').read_text())
    profile = pins.get('profile')
    if profile not in PROFILES:
        raise ValueError('unknown physical guest profile')
    route = PROFILES[profile]['route']
    library_pins = pins.get('z3_libraries', {})
    tool_pins = pins.get('tools', {})
    result = {'schema': 'semaprax.bend2-law-benchmark.proof-verdict-cache-guest.v2', 'status': 'incomplete', 'scope': SCOPE, 'profile': profile, 'route': route, 'tool_pins': tool_pins, 'z3_library_pins': library_pins, 'samples': []}
    try:
        profile_spec = PROFILES[profile]
        if platform.system() != 'Linux' or platform.machine() not in profile_spec['machine'] or os.sysconf('SC_PAGE_SIZE') != 4096:
            raise ValueError('capture guest architecture or page size differs from the selected physical profile')
        tool_paths = {name: Path('/tools') / name for name in profile_spec['tools']}
        if not (Path('/proc/sys/vm/drop_caches').is_file() and all(path.is_file() for path in tool_paths.values())):
            raise ValueError('pinned proof route tools or guest cache control are missing')
        if set(tool_pins) != set(profile_spec['tools']) or (profile.startswith('native_arm64') and not isinstance(library_pins, dict)):
            raise ValueError('profile pin manifest is incomplete')
        for name, expected in tool_pins.items():
            if sha(tool_paths[name]) != expected:
                raise ValueError(f'pinned tool digest differs: {name}')
        for name, expected in library_pins.items():
            if sha(Path('/tools') / name) != expected:
                raise ValueError(f'pinned Z3 library digest differs: {name}')
        for name, expected in SOURCES.items():
            if not name.startswith('project/' if route.startswith('semaprax') else 'fixtures/'):
                continue
            if sha(Path('/' + name)) != expected:
                raise ValueError(f'pinned proof source digest differs: {name}')
        env = dict(os.environ, BENDTT='/tools/bendtt', BEND_NO_TELEMETRY='1', DO_NOT_TRACK='1', HOME='/tmp/law16-home')
        Path(env['HOME']).mkdir(exist_ok=True)
        result['guest'] = {'platform': platform.platform(), 'kernel_release': platform.release(), 'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(), 'page_size': os.sysconf('SC_PAGE_SIZE')}
        result['source_pins'] = dict(pins)
        if route == 'bend_verdict':
            observed_bend = {p.relative_to('/bend2').as_posix(): sha(p) for p in sorted(Path('/bend2').rglob('*')) if p.is_file() and not p.is_symlink()}
            if observed_bend != pins.get('bend2_files'):
                raise ValueError('Bend source closure differs from the clean pinned commit')
            inv = {route: [tool_paths['bun'], tool_paths['bendtt']] + sorted(Path('/bend2').rglob('*')) + [Path('/fixtures/bend-boolean-negation-v1.bend')]}
        else:
            if pins.get('semaprax_commit') != SEMAPRAX_COMMIT:
                raise ValueError('SEMAPRAX source commit pin differs')
            inv = {route: [tool_paths['semaprax'], tool_paths['z3']] + sorted(Path('/tools').glob('libz3*.so*')) + sorted(Path('/project').rglob('*'))}
        result['file_inventory'] = {key: {str(p): {'bytes': p.stat().st_size, 'sha256': sha(p)} for p in paths if p.is_file()} for key, paths in inv.items()}
        result['tool_versions'] = {name: subprocess.run([str(tool_paths[name]), '--version'], capture_output=True, timeout=30).stdout.decode(errors='replace').strip() for name in profile_spec['tools']}
        expected_versions = {'bun': '1.2.5'} if route == 'bend_verdict' else {'z3': 'Z3 version 4.12.5 - 64 bit', 'semaprax': 'semaprax 0.8.0 (commit unknown)'}
        if any(result['tool_versions'].get(name) != version for name, version in expected_versions.items()) or any(not result['tool_versions'].get(name) for name in profile_spec['tools']):
            raise ValueError('pinned proof tool version output differs')
        result['preflight'] = {}
        for route in (route,):
            command = ROUTES[route]
            probe = run(route, 'preflight', 0, command)
            stdout = (OUT / probe['stdout']['path']).read_bytes()
            if probe['exit_code'] != 0 or probe['timed_out'] or (route == 'bend_verdict' and b'ALL PROOFS CHECK' not in stdout) or (route.startswith('semaprax') and b'app.negate' not in stdout):
                raise ValueError(f'proof success preflight failed: {route}')
            result['preflight'][route] = probe
        for ordinal in range(1, 31):
            for route in (route,):
                argv = ROUTES[route]
                paths = [p for p in inv[route] if p.is_file()]
                for path in paths:
                    with path.open('rb') as stream:
                        while stream.read(1024 * 1024):
                            pass
                before_reset = {str(p): resident(p) for p in paths}
                os.sync()
                Path('/proc/sys/vm/drop_caches').write_text('3\n')
                before_cold = {str(p): resident(p) for p in paths}
                if any(v['resident_pages'] != v['pages'] for v in before_reset.values()) or any(v['resident_pages'] for v in before_cold.values()):
                    raise ValueError(f'guest file residency was not demonstrably cold: {route} {ordinal}')
                cold = run(route, 'cold', ordinal, argv)
                for path in paths:
                    with path.open('rb') as stream:
                        while stream.read(1024 * 1024):
                            pass
                before_warm = {str(p): resident(p) for p in paths}
                if any(value['resident_pages'] != value['pages'] for value in before_warm.values()):
                    raise ValueError(f'guest warm residency is incomplete: {route} {ordinal}')
                warm = run(route, 'warm', ordinal, argv)
                expected = (cold, warm)
                if any(r['exit_code'] != 0 or r['timed_out'] for r in expected):
                    raise ValueError(f'proof route failed; successful proof sample required: {route} {ordinal}')
                for r in expected:
                    data = (OUT / r['stdout']['path']).read_bytes()
                    if not data:
                        raise ValueError(f'proof route returned empty stdout: {route} {ordinal}')
                    if route == 'bend_verdict' and b'ALL PROOFS CHECK' not in data:
                        raise ValueError('Bend verdict did not report ALL PROOFS CHECK')
                    if route.startswith('semaprax') and b'app.negate' not in data:
                        raise ValueError('SEMAPRAX output omitted selected app.negate obligation')
                result['samples'].append({'route': route, 'ordinal': ordinal, 'before_reset': before_reset, 'before_cold': before_cold, 'before_warm': before_warm, 'cold': cold, 'warm': warm})
        if len(result['samples']) != 30:
            raise ValueError('all thirty paired samples for the selected profile are required')
        validate_result(result, OUT)
        result['route_counts'] = {route: {'pairs': 30, 'cold': 30, 'warm': 30}}
        result['status'] = 'thirty_cold_warm_pairs_for_profile_observed'
    except Exception as error:
        result['reason'] = str(error)
    (OUT / 'guest-result.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    return 0 if result['status'] != 'incomplete' else 1

if __name__ == '__main__':
    raise SystemExit(main())
