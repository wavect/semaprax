#!/usr/bin/env python3
"""Run only SG24 documentation in a real, isolated VS Code Extension Host."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import runpy
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', required=True, type=Path)
    parser.add_argument('--vscode-app', required=True, type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    app = args.vscode_app.resolve(strict=True)
    cli = app / 'Contents/Resources/app/bin/code'
    with (app / 'Contents/Info.plist').open('rb') as metadata:
        executable = plistlib.load(metadata)['CFBundleExecutable']
    code = app / 'Contents/MacOS' / executable
    package = runpy.run_path(str(ROOT / 'scripts/graph-operational-vscode-host-evidence.py'))['package_vsix']
    environment = os.environ.copy()
    for key in ('ELECTRON_RUN_AS_NODE', 'VSCODE_IPC_HOOK_CLI'):
        environment.pop(key, None)
    # macOS Unix-domain IPC paths have a 103-byte limit; its default temp root
    # is too long once VS Code appends the user-data socket name.
    with tempfile.TemporaryDirectory(prefix='spx-doc-', dir='/tmp') as temporary:
        area = Path(temporary).resolve()
        workspace = area / 'workspace'
        shutil.copytree(ROOT / 'examples/calculator-project', workspace)
        user, extensions = area / 'user', area / 'extensions'
        (user / 'User').mkdir(parents=True)
        extensions.mkdir()
        vsix = area / 'semaprax.vsix'
        package(vsix)
        def run(command):
            result = subprocess.run(command, env=environment, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
            if result.returncode:
                raise RuntimeError(result.stdout.decode(errors='replace'))
            return result.stdout
        run([str(cli), f'--user-data-dir={user}', f'--extensions-dir={extensions}', '--install-extension', str(vsix), '--force'])
        installed = extensions / 'wavect.semaprax-0.1.0'
        (user / 'User/settings.json').write_text(json.dumps({'semaprax.compilerPath': str(compiler)}))
        environment.update({
            'SEMAPRAX_VSCODE_DOC_ONLY': '1', 'SEMAPRAX_VSCODE_COMPILER': str(compiler),
            'SEMAPRAX_VSCODE_MANIFEST': str(workspace / 'semaprax.toml'),
            'SEMAPRAX_VSCODE_EXPECTED_EXTENSION_PATH': str(installed.resolve(strict=True)),
        })
        output = run([str(code), f'--user-data-dir={user}', f'--extensions-dir={extensions}',
                      '--disable-workspace-trust', '--disable-gpu', '--disable-updates', '--skip-welcome', '--skip-release-notes',
                      f'--extensionDevelopmentPath={installed}',
                      f'--extensionTestsPath={ROOT / "editors/vscode/test/extension-host/index.js"}', str(workspace)])
        witnesses = re.findall(rb'^SEMAPRAX_PROJECT_DOC_HOST_RESULT=(\{[^\r\n]+\})$', output, re.MULTILINE)
        if len(witnesses) != 1:
            raise RuntimeError(output.decode(errors='replace'))
        witness = json.loads(witnesses[0])
        if witness['modules'] != ['core', 'app'] or witness['cli_bytes_match'] is not True:
            raise RuntimeError('incomplete Project documentation witness')
        print(json.dumps(witness, sort_keys=True))


if __name__ == '__main__':
    main()
