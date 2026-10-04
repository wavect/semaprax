#!/usr/bin/env python3
"""Bounded guest-only file-page-cache experiment; invoked by the host runner."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import time


def sha(path):
    return 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()


def residency(path):
    """mincore observes mapped file pages without faulting their contents in."""
    size = path.stat().st_size
    pages = (size + os.sysconf('SC_PAGE_SIZE') - 1) // os.sysconf('SC_PAGE_SIZE')
    libc = ctypes.CDLL(None, use_errno=True)
    libc.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_long]
    libc.mmap.restype = ctypes.c_void_p
    libc.mincore.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p]
    libc.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    with path.open('rb') as handle:
        address = libc.mmap(None, size, 0, 2, handle.fileno(), 0)  # PROT_NONE, MAP_PRIVATE
        if address == ctypes.c_void_p(-1).value:
            raise OSError(ctypes.get_errno(), 'mmap failed')
        try:
            vector = (ctypes.c_ubyte * pages)()
            if libc.mincore(address, size, vector):
                raise OSError(ctypes.get_errno(), 'mincore failed')
            return {'pages': pages, 'resident_pages': sum(value & 1 for value in vector)}
        finally:
            libc.munmap(address, size)


def capture(argv, output, name, environment):
    started = time.monotonic_ns()
    try:
        value = subprocess.run(argv, capture_output=True, env=environment, timeout=30)
        code, out, err, expired = value.returncode, value.stdout, value.stderr, False
    except subprocess.TimeoutExpired as error:
        code, out, err, expired = None, error.stdout or b'', error.stderr or b'', True
    elapsed = time.monotonic_ns() - started
    result = {'argv': argv, 'exit_code': code, 'timed_out': expired, 'elapsed_ns': elapsed}
    for channel, content in [('stdout', out), ('stderr', err)]:
        path = output / f'{name}.{channel}'
        path.write_bytes(content)
        result[channel] = {'path': path.name, 'bytes': len(content), 'sha256': sha(path)}
    return result


def main():
    output = Path('/output')
    request = json.loads((output / 'request.json').read_text())
    result = {'schema': 'semaprax.bend2-law-benchmark.guest-cache-worker.v1', 'status': 'incomplete',
              'samples': [], 'versions': {}, 'scope': 'Linux guest file page cache only; host and Rosetta caches unknown'}
    work = Path('/opt/law16')
    started = time.monotonic_ns()
    try:
        if platform.system() != 'Linux' or request['repetitions'] not in (1, 30):
            raise ValueError('requires Linux and one pilot or thirty repetitions')
        work.mkdir()
        for key, row in request['tools'].items():
            source = Path('/tools') / row['path']
            if sha(source) != row['sha256']:
                raise ValueError(f'tool digest differs: {key}')
            shutil.copyfile(source, work / key)
        subprocess.run(['dpkg-deb', '-x', str(work / 'libc6'), str(work / 'glibc')], check=True, capture_output=True)
        for name in ('semaprax', 'bun'):
            (work / name).chmod(0o755)
        for name in request['bend_files']:
            target = work / 'bend2' / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(Path('/bend2') / name, target)
            if sha(target) != request['bend_files'][name]:
                raise ValueError('Bend source digest differs')
        for name, row in request['inputs'].items():
            shutil.copyfile(Path('/inputs') / name, work / name)
            if sha(work / name) != row['sha256']:
                raise ValueError('source input digest differs')
        environment = dict(os.environ, BEND_NO_TELEMETRY='1', BUN_RUNTIME_TRANSPILER_CACHE_PATH='0', DO_NOT_TRACK='1', HOME=str(work / 'home'))
        Path(environment['HOME']).mkdir()
        lib = work / 'glibc/usr/lib/x86_64-linux-gnu'
        loader = lib / 'ld-linux-x86-64.so.2'
        prefix = [str(loader), '--library-path', str(lib)]
        commands = {
            'bend_ordinary': prefix + [str(work / 'bun'), str(work / 'bend2/main.ts'), str(work / 'candidate.bend')],
            'semaprax_check': prefix + [str(work / 'semaprax'), 'check', str(work / 'candidate.spx')],
        }
        for name in ('semaprax', 'bun'):
            row = capture(prefix + [str(work / name), '--version'], output, 'version-' + name, environment)
            result['versions'][name] = row
            if row['exit_code'] != 0 or (output / row['stdout']['path']).read_text().strip() != request['versions'][name]:
                raise ValueError('tool version mismatch')
        for lane, command in commands.items():
            row = capture(command, output, 'preflight-' + lane, environment)
            result.setdefault('preflight', {})[lane] = row
            if row['exit_code'] != 0:
                raise ValueError('source is not admitted by guest tool: ' + lane)
        result['provisioning_ns'] = time.monotonic_ns() - started
        result['guest'] = {'platform': platform.platform(), 'os_release': Path('/etc/os-release').read_text(),
                           'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(), 'page_size': os.sysconf('SC_PAGE_SIZE'),
                           'mounts': Path('/proc/mounts').read_text()}
        result['environment'] = {key: environment[key] for key in ('BEND_NO_TELEMETRY', 'BUN_RUNTIME_TRANSPILER_CACHE_PATH', 'DO_NOT_TRACK')}
        # Include exact executable, source closure and isolated glibc runtime in
        # the directly observed inventory. System libgcc and live Python pages
        # are outside that inventory; global guest eviction is not all-cache cold.
        shared = [path for path in lib.rglob('*') if path.is_file() and not path.is_symlink()]
        inventory = {
            'bend_ordinary': [work / 'bun', work / 'candidate.bend'] + sorted((work / 'bend2').rglob('*')) + shared,
            'semaprax_check': [work / 'semaprax', work / 'candidate.spx'] + shared,
        }
        inventory = {lane: [path for path in paths if path.is_file()] for lane, paths in inventory.items()}
        result['file_inventory'] = {lane: {str(path.relative_to(work)): {'bytes': path.stat().st_size, 'sha256': sha(path)} for path in paths} for lane, paths in inventory.items()}
        for ordinal in range(1, request['repetitions'] + 1):
            for lane in ('bend_ordinary', 'semaprax_check'):
                paths = inventory[lane]
                # Warm every inventoried page first, proving mincore can observe
                # it; then require every one to be evicted before timing.
                for path in paths:
                    path.read_bytes()
                before = {str(path.relative_to(work)): residency(path) for path in paths}
                reset_start = time.monotonic_ns()
                os.sync()
                Path('/proc/sys/vm/drop_caches').write_text('3\n')
                reset_ns = time.monotonic_ns() - reset_start
                cold = {str(path.relative_to(work)): residency(path) for path in paths}
                if not all(row['resident_pages'] == row['pages'] for row in before.values()) or any(row['resident_pages'] for row in cold.values()):
                    result['failed_cache_observation'] = {'before': before, 'after': cold, 'lane': lane, 'ordinal': ordinal}
                    raise ValueError('inventoried file pages are not demonstrably guest-cache cold')
                cold_sample = capture(commands[lane], output, f'{lane}-{ordinal:02d}-cold', environment)
                warm = {str(path.relative_to(work)): residency(path) for path in paths}
                warm_sample = capture(commands[lane], output, f'{lane}-{ordinal:02d}-warm', environment)
                row = {'lane': lane, 'ordinal': ordinal, 'reset_ns': reset_ns, 'before_reset': before,
                       'before_cold': cold, 'before_warm': warm, 'cold': cold_sample, 'warm': warm_sample}
                result['samples'].append(row)
                if cold_sample['exit_code'] != 0 or warm_sample['exit_code'] != 0 or not any(x['resident_pages'] for x in warm.values()):
                    raise ValueError('checker failed or warm residency was not observed')
                if (output / cold_sample['stdout']['path']).read_bytes() != (output / warm_sample['stdout']['path']).read_bytes():
                    raise ValueError('cold/warm outputs differ')
        result['status'] = 'pilot_guest_cache_observed' if request['repetitions'] == 1 else 'guest_cache_thirty_pairs_observed'
    except Exception as error:
        result['reason'] = str(error)
    (output / 'guest-result.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    return 0 if result['status'] != 'incomplete' else 1


if __name__ == '__main__':
    raise SystemExit(main())
