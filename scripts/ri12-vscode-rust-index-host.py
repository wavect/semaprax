#!/usr/bin/env python3
"""Run only the installed VS Code selected Rust index hover/CLI parity gate."""

import argparse
import json
import os
import re
import runpy
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
shared = runpy.run_path(str(ROOT / "scripts/graph-operational-vscode-host-evidence.py"))
MARKER = re.compile(rb"^SEMAPRAX_RUST_INDEX_HOST_RESULT=(\{[^\r\n]+\})$", re.MULTILINE)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--vscode-app", required=True)
    parser.add_argument("--compiler", required=True)
    args = parser.parse_args()
    app = Path(args.vscode_app).resolve(strict=True)
    compiler = Path(args.compiler).resolve(strict=True)
    code = app / "Contents/MacOS/Code"
    cli = app / "Contents/Resources/app/bin/code"
    product = app / "Contents/Resources/app/product.json"
    for path in (code, cli, product):
        if not path.is_file():
            raise ValueError(f"selected VS Code product is incomplete: {path}")
    if json.loads(product.read_text()).get("nameLong") != "Visual Studio Code":
        raise ValueError("selected app is not Visual Studio Code")
    with tempfile.TemporaryDirectory(prefix="semaprax-ri12-vscode-", dir="/private/tmp") as name:
        area = Path(name)
        workspace = area / "workspace"
        shutil.copytree(ROOT / "examples/calculator-project", workspace)
        policy = area / "policy.json"
        policy.write_bytes(shared["canonical"](shared["POLICY"]))
        user = area / "user"
        extensions = area / "extensions"
        (user / "User").mkdir(parents=True)
        extensions.mkdir()
        settings = {
            "semaprax.compilerPath": str(compiler),
            "semaprax.manifestPath": str(workspace / "semaprax.toml"),
            "semaprax.hostPolicyPath": str(policy),
        }
        (user / "User/settings.json").write_bytes(shared["canonical"](settings))
        vsix = area / "wavect.semaprax.vsix"
        shared["package_vsix"](vsix)
        shared["command"](
            [str(cli), f"--user-data-dir={user}", f"--extensions-dir={extensions}",
             "--install-extension", str(vsix), "--force"],
            "install exact RI-12 VSIX",
        )
        installed = extensions / "wavect.semaprax-0.1.0"
        if not installed.is_dir() or installed.is_symlink():
            raise ValueError("installed RI-12 VSIX is absent")
        environment = os.environ.copy()
        environment.update({
            "SEMAPRAX_VSCODE_COMPILER": str(compiler),
            "SEMAPRAX_VSCODE_MANIFEST": str(workspace / "semaprax.toml"),
            "SEMAPRAX_VSCODE_POLICY": str(policy),
            "SEMAPRAX_VSCODE_SOURCE": str(workspace / "src/core.spx"),
            "SEMAPRAX_VSCODE_EXPECTED_EXTENSION_PATH": str(installed.resolve(strict=True)),
            "SEMAPRAX_VSCODE_RUST_INDEX_ONLY": "1",
        })
        output = shared["command"](
            [str(code), f"--user-data-dir={user}", f"--extensions-dir={extensions}",
             "--disable-workspace-trust", "--disable-gpu", "--disable-updates",
             "--skip-welcome", "--skip-release-notes",
             f"--extensionDevelopmentPath={installed}",
             f"--extensionTestsPath={ROOT / 'editors/vscode/test/extension-host/index.js'}",
             str(workspace)],
            "RI-12 installed Extension Host Regex parity", env=environment,
        )
        matches = MARKER.findall(output)
        if len(matches) != 1:
            raise ValueError("RI-12 Extension Host did not report exactly one parity result")
        report = json.loads(matches[0])
        expected = {
            "schema": "semaprax.vscode-rust-index-host-result.v1",
            "app_name": "Visual Studio Code",
            "extension_path": str(installed.resolve(strict=True)),
            "installed_vsix": True,
            "selected_path": "regex::Regex::is_match",
            "signature": "fn is_match(&self, haystack: &str) -> bool",
            "receiver": "shared",
            "package": "regex 1.13.1",
            "cargo_alias": "regex_alias",
            "authority": {"build": False, "publication": False},
        }
        if report != expected:
            raise ValueError(f"RI-12 installed Extension Host parity differs: {report!r}")
        print("RI-12 installed Extension Host Regex parity: PASS 1/1")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, shared["Failure"]) as error:
        print(f"RI-12 Extension Host gate failed: {error}", file=sys.stderr)
        raise SystemExit(1)
