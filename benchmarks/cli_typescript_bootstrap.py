#!/usr/bin/env python3
"""Pinned, dependency-only TypeScript setup for optional CLI benchmark arms.

The template command is offline and never invokes npm. A human may run npm ci
in the external setup directory, then seal it into a receipt. Campaigns validate
the receipt and stage only its verified node_modules before prompting an agent.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

_spec = importlib.util.spec_from_file_location(
    "_semaprax_cli_dependencies", Path(__file__).resolve().with_name("webapp-tokens-v2") / "dependency_bundle.py")
if _spec is None or _spec.loader is None:
    raise RuntimeError("cannot load qualified dependency inventory helper")
_deps = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_deps)

SCHEMA = "semaprax.cli-typescript-bootstrap.v1"
PACKAGES = {
    "typescript": ("5.9.3", "sha512-jl1vZzPDinLr9eUt3J/t7V6FgNEw9QjvBPdysz9KfQDD41fQrC2Y4vKQdiaUpFT4bXlb1RHhLpp8wtm6M5TgSw=="),
    "@types/node": ("22.20.5", "sha512-U2+DNr+wSjpsTS/wZGYHq7GcwfuSmKiKvoPvK22zwTlRhU91yOniN4qRR5KhIjvif7ysw/dz/hKmfDH0Ris4aA=="),
    "undici-types": ("6.21.0", "sha512-iwDZqg0QAGrg9Rav5H4n0M64c3mkR59cJ6wQp+7C4nI0gsmExaedaYLNO44eT4AtBBwjbTiGPMlt2Md0T9H9JQ=="),
}
TEMPLATE = {
    "name": "semaprax-cli-typescript-tooling",
    "version": "1.0.0",
    "private": True,
    "license": "UNLICENSED",
    "devDependencies": {name: version for name, (version, _) in sorted(PACKAGES.items())},
}
LOCK = {
    "name": TEMPLATE["name"], "version": TEMPLATE["version"], "lockfileVersion": 3,
    "requires": True,
    "packages": {
        "": {"name": TEMPLATE["name"], "version": TEMPLATE["version"], "license": "UNLICENSED",
             "devDependencies": TEMPLATE["devDependencies"]},
        **{f"node_modules/{name}": {
            "version": version,
            "resolved": f"https://registry.npmjs.org/{name}/-/" +
                        (f"node-{version}.tgz" if name == "@types/node" else f"{name.split('/')[-1]}-{version}.tgz"),
            "integrity": integrity, "dev": True,
            "license": {"typescript": "Apache-2.0", "@types/node": "MIT", "undici-types": "MIT"}[name],
            **({"dependencies": {"undici-types": "~6.21.0"}} if name == "@types/node" else {}),
            **({"bin": {"tsc": "bin/tsc", "tsserver": "bin/tsserver"}, "engines": {"node": ">=14.17"}}
               if name == "typescript" else {}),
        } for name, (version, integrity) in PACKAGES.items()},
    },
}
HELPER_HASH = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
DEPENDENCY_HELPER_HASH = hashlib.sha256(Path(_spec.origin).read_bytes()).hexdigest()
REPO_ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    return _deps.digest(path)


def write_template(destination: Path) -> None:
    if (destination.absolute() != destination.resolve(strict=False)
            or destination.resolve(strict=False).is_relative_to(REPO_ROOT)):
        raise ValueError("bootstrap template must be outside the repository")
    if destination.exists() or destination.is_symlink():
        raise ValueError("bootstrap template destination must be new")
    destination.mkdir(parents=True)
    (destination / "package.json").write_text(json.dumps(TEMPLATE, indent=2, sort_keys=True) + "\n")
    (destination / "package-lock.json").write_text(json.dumps(LOCK, indent=2, sort_keys=True) + "\n")
    (destination / "BOOTSTRAP.md").write_text(
        "Run `npm ci --ignore-scripts --no-audit --no-fund` in this directory, then run the campaign helper's `seal` command.\n"
        "This directory must remain outside the candidate and repository. It supplies only verified node_modules.\n")


def runtime(node_binary: str, npm_binary: str, root: Path) -> dict[str, Any]:
    node = shutil.which(node_binary) or node_binary
    npm = shutil.which(npm_binary) or npm_binary
    node_path, npm_path = Path(node).resolve(strict=True), Path(npm).resolve(strict=True)
    node_out = subprocess.run([str(node_path), "--version"], check=True, capture_output=True, text=True, timeout=20).stdout.strip()
    npm_out = subprocess.run([str(node_path), str(npm_path), "--version"], check=True, capture_output=True, text=True, timeout=20).stdout.strip()
    tsc_path = root / "node_modules/.bin/tsc"
    if tsc_path.is_symlink():
        target = tsc_path.resolve(strict=True)
        try:
            target.relative_to((root / "node_modules").resolve(strict=True))
        except ValueError as error:
            raise ValueError("tsc launcher escapes node_modules") from error
    if not tsc_path.exists():
        raise ValueError("bootstrap is missing node_modules/.bin/tsc")
    tsc_out = subprocess.run([str(node_path), str(root / "node_modules/typescript/bin/tsc"), "--version"], cwd=root, check=True, capture_output=True,
                             text=True, timeout=20).stdout.strip()
    if tsc_out != "Version " + PACKAGES["typescript"][0]:
        raise ValueError("observed TypeScript compiler version differs from pin")
    return {"node_binary_sha256": sha(node_path), "npm_binary_sha256": sha(npm_path),
            "node_version": node_out, "npm_version": npm_out, "tsc_version": tsc_out}


def inventory(root: Path) -> dict[str, list[dict[str, Any]]]:
    for name in ("package.json", "package-lock.json", "BOOTSTRAP.md"):
        path = root / name
        if path.is_symlink() or (path.exists() and not path.is_file()):
            raise ValueError(f"bootstrap metadata must be regular files: {name}")
    for relative in ("node_modules", "node_modules/.bin", "node_modules/@types",
                     "node_modules/typescript", "node_modules/undici-types"):
        path = root / relative
        if path.is_symlink() or not path.is_dir():
            raise ValueError(f"bootstrap structure must use real directories: {relative}")
    value = _deps.dependency_inventory(root, allow_other=True)
    if set(p.name for p in root.iterdir()) - {"node_modules", "package.json", "package-lock.json", "BOOTSTRAP.md"}:
        raise ValueError("bootstrap root contains an unsupported path")
    expected_roots = {"node_modules/.bin", "node_modules/@types", "node_modules/typescript",
                      "node_modules/undici-types", "node_modules/.package-lock.json"}
    roots = {p.relative_to(root).as_posix() for p in (root / "node_modules").iterdir()}
    if roots != expected_roots:
        raise ValueError("bootstrap must contain exactly the three pinned packages and .bin")
    if {p.name for p in (root / "node_modules/@types").iterdir()} != {"node"}:
        raise ValueError("@types may contain only the pinned node declarations")
    if {p.name for p in (root / "node_modules/.bin").iterdir()} != {"tsc", "tsserver"}:
        raise ValueError(".bin may contain only the two TypeScript launchers")
    for name, (version, _) in PACKAGES.items():
        path = root / "node_modules" / name / "package.json"
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"missing regular package metadata for {name}")
        pkg = json.loads(path.read_text(encoding="utf-8"))
        if pkg.get("name") != name or pkg.get("version") != version:
            raise ValueError(f"installed package version differs from pin: {name}")
    links = {row["path"]: row["target"] for row in value["symlinks"]}
    if links != {"node_modules/.bin/tsc": "../typescript/bin/tsc",
                 "node_modules/.bin/tsserver": "../typescript/bin/tsserver"}:
        raise ValueError("bootstrap contains unexpected or invalid package launcher links")
    value["directories"] = sorted(
        path.relative_to(root).as_posix() for path in (root / "node_modules").rglob("*")
        if path.is_dir() and not path.is_symlink())
    return value


def candidate_inventory(candidate: Path) -> dict[str, list[dict[str, Any]]]:
    """Inventory only the staged node_modules while retaining other candidate files."""
    value = _deps.dependency_inventory(candidate, allow_other=True)
    root = candidate / "node_modules"
    value["directories"] = sorted(
        path.relative_to(candidate).as_posix() for path in root.rglob("*")
        if path.is_dir() and not path.is_symlink()) if root.is_dir() and not root.is_symlink() else []
    return value


def dependency_snapshot(candidate: Path) -> dict[str, Any]:
    roots = []
    files, links, directories = [], [], []
    for root_name in ("node_modules", ".cache"):
        root = candidate / root_name
        if root.is_symlink():
            roots.append({"path": root_name, "kind": "symlink", "target": os.readlink(root)})
            continue
        if not root.exists():
            continue
        if not root.is_dir():
            roots.append({"path": root_name, "kind": "unsupported"})
            continue
        roots.append({"path": root_name, "kind": "directory"})
        for current, names, filenames in os.walk(root, topdown=True, followlinks=False):
            base = Path(current)
            for name in sorted(names):
                path = base / name
                relative = path.relative_to(candidate).as_posix()
                mode = path.lstat().st_mode
                if stat.S_ISLNK(mode):
                    links.append({"path": relative, "target": os.readlink(path)})
                elif stat.S_ISDIR(mode):
                    directories.append(relative)
                else:
                    files.append({"path": relative, "kind": "unsupported"})
            names[:] = [name for name in names if not (base / name).is_symlink()]
            for name in sorted(filenames):
                path = base / name
                relative = path.relative_to(candidate).as_posix()
                mode = path.lstat().st_mode
                if stat.S_ISLNK(mode):
                    links.append({"path": relative, "target": os.readlink(path)})
                elif stat.S_ISREG(mode):
                    files.append({"path": relative, "sha256": sha(path), "bytes": path.stat().st_size})
                else:
                    files.append({"path": relative, "kind": "unsupported"})
    value = {"status": "captured", "roots": roots, "files": files, "symlinks": links, "directories": directories}
    value["inventory_sha256"] = _deps.dependency_fingerprint(value)
    return value


def preserve_dependency_evidence(candidate: Path, destination: Path) -> dict[str, Any]:
    destination.mkdir(parents=True, exist_ok=False)
    before = dependency_snapshot(candidate)
    for root_name in ("node_modules", ".cache"):
        source = candidate / root_name
        if source.is_dir() and not source.is_symlink():
            shutil.copytree(source, destination / root_name, symlinks=True)
    after = dependency_snapshot(candidate)
    copied = dependency_snapshot(destination)
    snapshot = {"before_copy": before, "after_copy": after, "copied_tree": copied,
                "copy_consistent": before == after == copied}
    (destination / "inventory.json").write_text(json.dumps(snapshot, indent=2, sort_keys=True) + "\n",
                                                 encoding="utf-8")
    return {"path": str(destination), "snapshot": snapshot}


def verify_staged(candidate: Path, expected: dict[str, Any], *,
                  evidence_path: Path | None = None) -> tuple[bool, dict[str, Any]]:
    try:
        observed = candidate_inventory(candidate)
    except Exception as error:
        try:
            snapshot: dict[str, Any] = dependency_snapshot(candidate)
        except Exception as snapshot_error:
            snapshot = {"status": "unavailable", "error": str(snapshot_error)}
        result: dict[str, Any] = {"status": "invalid", "error": str(error),
                                  "expected_inventory_sha256": _deps.dependency_fingerprint(expected),
                                  "observed_snapshot": snapshot}
    else:
        if observed == expected:
            return True, {"status": "verified", "inventory_sha256": _deps.dependency_fingerprint(observed)}
        result = {"status": "drift", "expected_inventory_sha256": _deps.dependency_fingerprint(expected),
                  "observed_inventory": observed,
                  "observed_inventory_sha256": _deps.dependency_fingerprint(observed)}
    if evidence_path is not None:
        try:
            result["preserved"] = preserve_dependency_evidence(candidate, evidence_path)
        except Exception as error:
            result["preservation_error"] = str(error)
    return False, result


def seal(root: Path, receipt_path: Path, node_binary: str = "node", npm_binary: str = "npm") -> dict[str, Any]:
    if root.is_symlink():
        raise ValueError("bootstrap root must not be a symlink")
    original_root = root.expanduser()
    original_receipt = receipt_path.expanduser()
    if original_receipt.exists() or original_receipt.is_symlink():
        raise ValueError("bootstrap receipt destination must be new")
    root = original_root.resolve(strict=True)
    receipt_resolved = original_receipt.resolve(strict=False)
    if (original_root.absolute() != root or original_receipt.absolute() != receipt_resolved
            or root.is_relative_to(REPO_ROOT) or receipt_resolved.is_relative_to(REPO_ROOT)
            or receipt_resolved.is_relative_to(root)):
        raise ValueError("bootstrap setup and receipt must be outside the repository")
    manifest, lock = root / "package.json", root / "package-lock.json"
    if any(path.is_symlink() or not path.is_file() for path in (manifest, lock)):
        raise ValueError("bootstrap manifest and lock must be regular files")
    if json.loads(manifest.read_text()) != TEMPLATE or json.loads(lock.read_text()) != LOCK:
        raise ValueError("bootstrap package manifest or lock differs from the exact pinned template")
    tree = inventory(root)
    observed = runtime(node_binary, npm_binary, root)
    receipt = {"schema": SCHEMA, "root": str(root), "package_json_sha256": sha(manifest),
              "package_lock_sha256": sha(lock), "inventory": tree,
              "inventory_sha256": _deps.dependency_fingerprint(tree), "packages": {k: v[0] for k, v in PACKAGES.items()},
              "runtime": observed, "helper_sha256": HELPER_HASH,
              "dependency_helper_sha256": DEPENDENCY_HELPER_HASH,
              "supplied_paths": ["node_modules"], "application_source_supplied": False,
              "package_manifest_supplied": False}
    receipt_path = receipt_path.resolve()
    if receipt_path.is_relative_to(root):
        raise ValueError("bootstrap receipt must be outside the dependency root")
    receipt_path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return receipt


def validate(receipt_path: Path, node_binary: str = "node", npm_binary: str = "npm") -> dict[str, Any]:
    if receipt_path.is_symlink() or not receipt_path.is_file():
        raise ValueError("bootstrap receipt must be a regular file")
    original_receipt_path = receipt_path
    receipt_path = receipt_path.resolve(strict=True)
    if (original_receipt_path.expanduser().absolute() != receipt_path
            or receipt_path.is_relative_to(REPO_ROOT)):
        raise ValueError("bootstrap receipt must be canonical and outside the repository")
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    root = Path(receipt.get("root", ""))
    if (receipt.get("schema") != SCHEMA or receipt.get("helper_sha256") != HELPER_HASH
            or receipt.get("dependency_helper_sha256") != DEPENDENCY_HELPER_HASH):
        raise ValueError("unsupported bootstrap receipt or helper drift")
    if root.is_symlink() or not root.is_dir() or not root.is_absolute() or str(root) != str(root.resolve(strict=True)):
        raise ValueError("bootstrap root must be a real directory")
    if root.is_relative_to(REPO_ROOT):
        raise ValueError("bootstrap root must be outside the repository")
    if receipt_path.is_relative_to(root):
        raise ValueError("bootstrap receipt must be outside the dependency root")
    manifest, lock = root / "package.json", root / "package-lock.json"
    if any(path.is_symlink() or not path.is_file() for path in (manifest, lock)):
        raise ValueError("bootstrap manifest and lock must be regular files")
    if sha(manifest) != receipt.get("package_json_sha256") or sha(lock) != receipt.get("package_lock_sha256"):
        raise ValueError("bootstrap manifest or lock drift")
    if json.loads((root / "package.json").read_text()) != TEMPLATE or json.loads((root / "package-lock.json").read_text()) != LOCK:
        raise ValueError("bootstrap manifest or lock differs from pinned template")
    tree = inventory(root)
    if tree != receipt.get("inventory") or _deps.dependency_fingerprint(tree) != receipt.get("inventory_sha256"):
        raise ValueError("bootstrap dependency inventory or hash drift")
    if runtime(node_binary, npm_binary, root) != receipt.get("runtime"):
        raise ValueError("observed Node, npm, or TypeScript version drift")
    return receipt


def verify_plan(tooling: dict[str, Any] | None) -> dict[str, Any] | None:
    if tooling is None:
        return None
    receipt_path = Path(tooling["receipt_path"])
    receipt = validate(receipt_path, tooling["node_binary"], tooling["npm_binary"])
    if (sha(receipt_path) != tooling["receipt_sha256"]
            or receipt["inventory_sha256"] != tooling["inventory_sha256"]
            or receipt["runtime"] != tooling["runtime"]
            or receipt["package_json_sha256"] != tooling["package_json_sha256"]
            or receipt["package_lock_sha256"] != tooling["package_lock_sha256"]
            or receipt["helper_sha256"] != tooling["helper_sha256"]
            or receipt["dependency_helper_sha256"] != tooling["dependency_helper_sha256"]):
        raise ValueError("TypeScript bootstrap changed after campaign planning")
    return receipt


def stage(receipt_path: Path, candidate: Path, node_binary: str, npm_binary: str) -> dict[str, Any]:
    receipt = validate(receipt_path, node_binary, npm_binary)
    candidate_resolved = candidate.resolve(strict=False)
    source_root = Path(receipt["root"])
    if (candidate_resolved == source_root or candidate_resolved.is_relative_to(source_root)
            or source_root.is_relative_to(candidate_resolved)):
        raise ValueError("bootstrap setup and candidate must be disjoint trees")
    candidate.mkdir(parents=True, exist_ok=True)
    source = Path(receipt["root"])
    target = candidate / "node_modules"
    if target.exists() or target.is_symlink():
        raise ValueError("candidate already contains node_modules")
    if inventory(source) != receipt["inventory"]:
        raise ValueError("bootstrap dependency inventory drifted before staging")
    with tempfile.TemporaryDirectory(prefix="semaprax-cli-ts-bundle-") as temporary:
        bundle = Path(temporary)
        shutil.copytree(source / "node_modules", bundle / "node_modules", symlinks=True)
        if inventory(bundle) != receipt["inventory"]:
            raise ValueError("bootstrap dependency inventory changed while staging")
        _deps.copy_dependency_bundle(bundle, candidate,
                                     {key: receipt["inventory"][key] for key in ("files", "symlinks")})
    if candidate_inventory(candidate) != receipt["inventory"] or inventory(source) != receipt["inventory"]:
        shutil.rmtree(target)
        raise ValueError("bootstrap dependency inventory drifted during staging")
    return {"receipt_sha256": sha(receipt_path), "inventory_sha256": receipt["inventory_sha256"],
            "runtime": receipt["runtime"], "packages": receipt["packages"],
            "supplied_paths": ["node_modules"], "application_source_supplied": False,
            "package_manifest_supplied": False}


def prompt_note(receipt: dict[str, Any]) -> str:
    return ("A private writable node_modules tree containing TypeScript 5.9.3 and @types/node 22.20.5 is present. "
            "Use node_modules/.bin/tsc; author package.json, tsconfig.json, scripts, tests, and all application code yourself. "
            "No npm install is required.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    template = sub.add_parser("template"); template.add_argument("--destination", type=Path, required=True)
    seal_parser = sub.add_parser("seal"); seal_parser.add_argument("--root", type=Path, required=True)
    seal_parser.add_argument("--receipt", type=Path, required=True); seal_parser.add_argument("--node-binary", default="node")
    seal_parser.add_argument("--npm-binary", default="npm")
    args = parser.parse_args()
    if args.action == "template":
        write_template(args.destination)
    else:
        result = seal(args.root, args.receipt, args.node_binary, args.npm_binary)
        print(json.dumps({"receipt": str(args.receipt.resolve()), "inventory_sha256": result["inventory_sha256"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
