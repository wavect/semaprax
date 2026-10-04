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
import shutil
import stat

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
ROUTES = {
    'bend_verdict': ['/opt/law16/tools/bun', '/opt/law16/bend2/main.ts', '/opt/law16/fixtures/bend-boolean-negation-v1.bend', '--verdict'],
    'semaprax_z3_project_proof_check': ['/tools/semaprax', 'project-proof-check', '/project/semaprax.toml', '--tool', 'z3', '--executable', '/tools/z3', '--version-line', 'Z3 version 4.12.5 - 64 bit', '--host-profile', 'trusted-local', '--source', 'src/app.spx', '--declaration', 'app.negate', '--ensures', '0'],
}
PROFILES = {
    'pinned_x86_rosetta_bend_verdict': {'route': 'bend_verdict', 'machine': ('x86_64',), 'tools': ('bun', 'bendtt')},
    'native_arm64_z3_source_obligation': {'route': 'z3_source_obligation', 'machine': ('aarch64', 'arm64'), 'tools': ('z3',)},
}
ROUTES['z3_source_obligation'] = ['/opt/law16/tools/z3', '-smt2', '/opt/law16/obligation.smt2']

def sha(path):
    return 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()

def validate_source_obligation(pins):
    script_path = Path('/opt/law16/obligation.smt2')
    source_script = Path('/opt/law16/source-script')
    source_path = Path('/opt/law16/source/src/app.spx')
    if sha(script_path) != pins['obligation']['sha256']:
        raise ValueError('generated SMT-LIB bytes differ from the pinned obligation')
    if sha(source_script) != pins['source_script']['sha256']:
        raise ValueError('retained source renderer digest differs')
    if sha(source_path) != pins['source']['sha256'] or pins['source']['sha256'] != SOURCES['project/src/app.spx']:
        raise ValueError('source file differs from the fixed admitted fixture')
    script = script_path.read_bytes()
    if not script.endswith(b'(check-sat)\n') or b'(get-model)' in script:
        raise ValueError('solver input is not the exact stripped check-sat obligation')
    return {'source_commit': pins['source_commit'], 'renderer_version': pins['renderer_version'],
            'source_digest': pins['source']['sha256'], 'obligation_sha256': pins['obligation']['sha256'],
            'source_renderer_sha256': pins['source_script']['sha256']}

def z3_success(row, stdout):
    nonempty = [line.strip() for line in stdout.decode(errors='replace').splitlines() if line.strip()]
    if not nonempty or nonempty[0] != 'unsat' or row.get('timed_out'):
        return False
    code = row.get('exit_code')
    return code == 0 and len(nonempty) == 1

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
        child = subprocess.run(argv, capture_output=True, timeout=120, env=dict(os.environ, BENDTT='/opt/law16/tools/bendtt', BEND_NO_TELEMETRY='1', BUN_RUNTIME_TRANSPILER_CACHE_PATH='0', DO_NOT_TRACK='1', LD_LIBRARY_PATH='/opt/law16/tools', HOME='/tmp/law16-home'))
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
    pinned_rows = [('/opt/law16/tools/' + name, digest, route) for name, digest in tool_pins.items()]
    pinned_rows += [('/opt/law16/tools/' + name, digest, route) for name, digest in library_pins.items()]
    source_prefix = 'project/' if route.startswith('semaprax') else 'fixtures/'
    if route == 'z3_source_obligation':
        pins = result.get('source_pins', {})
        pinned_rows += [('/opt/law16/obligation.smt2', pins['obligation']['sha256'], route),
                        ('/opt/law16/source-script', pins['source_script']['sha256'], route),
                        ('/opt/law16/source/src/app.spx', pins['source']['sha256'], route)]
    else:
        pinned_rows += [('/opt/law16/' + name, digest, route) for name, digest in SOURCES.items() if name.startswith(source_prefix)]
    if route == 'bend_verdict':
        pinned_rows += [('/opt/law16/bend2/' + name, digest, route) for name, digest in result.get('source_pins', {}).get('bend2_files', {}).items()]
    for name, expected_digest, route in pinned_rows:
        if result.get('file_inventory', {}).get(route, {}).get(name, {}).get('sha256') != expected_digest:
            raise ValueError('pinned tool, library, or source digest is absent from residency inventory')
    preflight = result.get('preflight', {}).get(route)
    if not isinstance(preflight, dict) or preflight.get('timed_out'):
        raise ValueError('successful proof preflight is missing')
    for channel in ('stdout', 'stderr'):
        ref = preflight.get(channel, {})
        path = output / ref.get('path', '')
        if (not isinstance(ref.get('path'), str) or Path(ref['path']).name != ref['path'] or not path.is_file()
                or path.is_symlink() or path.stat().st_size != ref.get('bytes') or sha(path) != ref.get('sha256')):
            raise ValueError('preflight raw proof stream is missing or drifted')
    preflight_stdout = (output / preflight['stdout']['path']).read_bytes()
    if ((route == 'bend_verdict' and (preflight.get('exit_code') != 0 or b'ALL PROOFS CHECK' not in preflight_stdout))
            or (route.startswith('semaprax') and b'app.negate' not in preflight_stdout)
            or (route == 'z3_source_obligation' and not z3_success(preflight, preflight_stdout))):
        raise ValueError('preflight did not report the expected successful proof verdict')
    for sample in result['samples']:
        inventory = result.get('file_inventory', {}).get(sample['route'])
        if not isinstance(inventory, dict) or not inventory:
            raise ValueError('residency inventory is missing for a proof route')
        for field in ('before_reset', 'before_cold', 'before_warm'):
            if set(sample.get(field, {})) != set(inventory):
                raise ValueError('residency receipt does not cover the complete file inventory')
        for name, item in inventory.items():
            if not isinstance(item.get('mode'), int) or item['mode'] < 0 or item['mode'] > 0o777:
                raise ValueError('file inventory mode receipt is malformed')
            pages = (item['bytes'] + 4095) // 4096
            reset, cold, warm = sample['before_reset'][name], sample['before_cold'][name], sample['before_warm'][name]
            if (reset.get('pages') != pages or reset.get('resident_pages') != pages
                    or cold.get('pages') != pages or cold.get('resident_pages') != 0
                    or warm.get('pages') != pages or warm.get('resident_pages') != pages):
                raise ValueError('guest pages were not observed warm then fully cold')
        for state in ('cold', 'warm'):
            row = sample.get(state, {})
            if ((route != 'z3_source_obligation' and row.get('exit_code') != 0)
                    or row.get('timed_out') or row.get('elapsed_ns', 0) <= 0):
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
            if route == 'z3_source_obligation' and not z3_success(row, stdout):
                raise ValueError('Z3 source obligation did not return the expected unsat result')
            if (not stdout or (sample['route'] == 'bend_verdict' and b'ALL PROOFS CHECK' not in stdout)
                    or (sample['route'].startswith('semaprax') and b'app.negate' not in stdout)
                    or (sample['route'] == 'z3_source_obligation' and not z3_success(row, stdout))):
                raise ValueError('successful proof verdict is not present in raw stdout')
    return True

def main():
    pins = json.loads(Path('/capture/source-pins.json').read_text())
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
        local = Path('/opt/law16')
        local.mkdir(exist_ok=True)
        tool_dir = local / 'tools'; tool_dir.mkdir(exist_ok=True)
        tool_paths = {}
        for name in profile_spec['tools']:
            mounted = Path('/tools') / name
            target = tool_dir / name
            shutil.copyfile(mounted, target)
            target.chmod(0o755)
            if sha(mounted) != sha(target) or stat.S_IMODE(mounted.stat().st_mode) != stat.S_IMODE(target.stat().st_mode):
                raise ValueError(f'guest-local tool copy differs in bytes or mode: {name}')
            tool_paths[name] = target
        if route == 'bend_verdict':
            shutil.copytree('/bend2', local / 'bend2', symlinks=True)
            fixture_dir = local / 'fixtures'; fixture_dir.mkdir(exist_ok=True)
            shutil.copyfile('/fixtures/bend-boolean-negation-v1.bend', fixture_dir / 'bend-boolean-negation-v1.bend')
        else:
            for name, source in [('obligation.smt2', '/capture/obligation.smt2'), ('source-script', '/capture/source-script')]:
                shutil.copyfile(source, local / name)
            shutil.copytree('/capture/source', local / 'source')
        if not (Path('/proc/sys/vm/drop_caches').is_file() and all(path.is_file() for path in tool_paths.values())):
            raise ValueError('pinned proof route tools or guest cache control are missing')
        if set(tool_pins) != set(profile_spec['tools']) or (profile.startswith('native_arm64') and not isinstance(library_pins, dict)):
            raise ValueError('profile pin manifest is incomplete')
        for name, expected in tool_pins.items():
            if sha(tool_paths[name]) != expected:
                raise ValueError(f'pinned tool digest differs: {name}')
        for name, expected in library_pins.items():
            if sha(tool_dir / name) != expected:
                raise ValueError(f'pinned Z3 library digest differs: {name}')
        for name, expected in SOURCES.items():
            if route == 'z3_source_obligation' or not name.startswith('project/' if route.startswith('semaprax') else 'fixtures/'):
                continue
            if sha(Path('/' + name)) != expected:
                raise ValueError(f'pinned proof source digest differs: {name}')
        env = dict(os.environ, BENDTT='/opt/law16/tools/bendtt', BEND_NO_TELEMETRY='1', DO_NOT_TRACK='1', HOME='/tmp/law16-home')
        Path(env['HOME']).mkdir(exist_ok=True)
        result['guest'] = {'platform': platform.platform(), 'kernel_release': platform.release(), 'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(), 'page_size': os.sysconf('SC_PAGE_SIZE')}
        result['source_pins'] = dict(pins)
        if route == 'bend_verdict':
            observed_bend = {p.relative_to(local / 'bend2').as_posix(): sha(p) for p in sorted((local / 'bend2').rglob('*')) if p.is_file() and not p.is_symlink()}
            if observed_bend != pins.get('bend2_files'):
                raise ValueError('Bend source closure differs from the clean pinned commit')
            inv = {route: [tool_paths['bun'], tool_paths['bendtt']] + sorted((local / 'bend2').rglob('*')) + [local / 'fixtures/bend-boolean-negation-v1.bend']}
        elif route == 'semaprax_z3_project_proof_check':
            if pins.get('semaprax_commit') != SEMAPRAX_COMMIT:
                raise ValueError('SEMAPRAX source commit pin differs')
            inv = {route: [tool_paths['semaprax'], tool_paths['z3']] + sorted(tool_dir.glob('libz3*.so*')) + sorted(Path('/project').rglob('*'))}
        else:
            result['proof_observation'] = validate_source_obligation(pins)
            source_path = local / 'source/src/app.spx'
            if sha(source_path) != SOURCES['project/src/app.spx'] or sha(source_path) != pins['source']['sha256']:
                raise ValueError('source file differs from the fixed admitted fixture')
            inv = {route: [tool_paths['z3'], local / 'obligation.smt2', local / 'source-script', source_path]}
        result['file_inventory'] = {key: {str(p): {'bytes': p.stat().st_size, 'sha256': sha(p), 'mode': stat.S_IMODE(p.stat().st_mode)} for p in paths if p.is_file()} for key, paths in inv.items()}
        result['tool_versions'] = {name: subprocess.run([str(tool_paths[name]), '--version'], capture_output=True, timeout=30).stdout.decode(errors='replace').strip() for name in profile_spec['tools']}
        expected_versions = {'bun': '1.2.5'} if route == 'bend_verdict' else {'z3': 'Z3 version 4.12.5 - 64 bit'}
        if route == 'semaprax_z3_project_proof_check':
            expected_versions['semaprax'] = 'semaprax 0.8.0 (commit unknown)'
        if any(result['tool_versions'].get(name) != version for name, version in expected_versions.items()):
            raise ValueError('pinned proof tool version output differs')
        result['preflight'] = {}
        for route in (route,):
            command = ROUTES[route]
            probe = run(route, 'preflight', 0, command)
            stdout = (OUT / probe['stdout']['path']).read_bytes()
            if ((route != 'z3_source_obligation' and probe['exit_code'] != 0)
                    or (route == 'z3_source_obligation' and not z3_success(probe, stdout))
                    or probe['timed_out'] or (route == 'bend_verdict' and b'ALL PROOFS CHECK' not in stdout)
                    or (route.startswith('semaprax') and b'app.negate' not in stdout)):
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
                    raise ValueError(f'guest file residency was not demonstrably cold: {route} {ordinal}: ' + repr({'reset_incomplete': {name: value for name, value in before_reset.items() if value['resident_pages'] != value['pages']}, 'cold_resident': {name: value for name, value in before_cold.items() if value['resident_pages']}}))
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
                if any((r['exit_code'] != 0 and route != 'z3_source_obligation') or r['timed_out'] for r in expected):
                    raise ValueError(f'proof route failed; successful proof sample required: {route} {ordinal}')
                for r in expected:
                    data = (OUT / r['stdout']['path']).read_bytes()
                    if not data:
                        raise ValueError(f'proof route returned empty stdout: {route} {ordinal}')
                    if route == 'bend_verdict' and b'ALL PROOFS CHECK' not in data:
                        raise ValueError('Bend verdict did not report ALL PROOFS CHECK')
                    if route.startswith('semaprax') and b'app.negate' not in data:
                        raise ValueError('SEMAPRAX output omitted selected app.negate obligation')
                    if route == 'z3_source_obligation' and not z3_success(r, data):
                        raise ValueError('Z3 source obligation did not return expected unsat result')
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
