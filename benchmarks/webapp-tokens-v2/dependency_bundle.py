"""Closed dependency inventories and private copies shared by campaign tooling.

Legacy rescore receipts keep their original inventory and validation semantics.
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
from pathlib import Path
from typing import Any


def digest(path: Path) -> str:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected regular file: {path}")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dependency_inventory(bundle: Path, *, allow_other: bool = False) -> dict[str, list[dict[str, Any]]]:
    """Return the closed, link-preserving dependency bundle inventory."""
    if bundle.is_symlink() or not bundle.is_dir():
        raise ValueError("dependency bundle must be a real directory")
    bundle = bundle.resolve(strict=True)
    allowed = {"node_modules", ".cache"}
    children = sorted(bundle.iterdir(), key=lambda path: path.name)
    top = [path.name for path in children]
    if not top or (not allow_other and set(top) - allowed):
        raise ValueError("dependency bundle has an unsupported top-level path")
    roots = [path for path in children if path.name in allowed]
    if any(path.is_symlink() or not path.is_dir() for path in roots):
        raise ValueError("dependency bundle top-level paths must be real directories")
    def link_target(path: Path) -> str:
        target = os.readlink(path)
        if Path(target).is_absolute():
            raise ValueError("dependency bundle has an external symlink")
        try:
            resolved = (path.parent / target).resolve(strict=False)
            relative = resolved.relative_to(bundle)
        except ValueError as error:
            raise ValueError("dependency bundle has an external symlink") from error
        if resolved == bundle or not relative.parts or relative.parts[0] not in allowed:
            raise ValueError("dependency bundle has a root symlink")
        return target
    files: list[dict[str, Any]] = []
    links: list[dict[str, Any]] = []
    for top_root in roots:
      for current, directories, names in os.walk(top_root, topdown=True, followlinks=False):
          root = Path(current)
          retained: list[str] = []
          for name in sorted(directories):
              path = root / name
              relative = path.relative_to(bundle).as_posix()
              mode = path.lstat().st_mode
              if path.is_symlink():
                  target = link_target(path)
                  links.append({"path": relative, "target": target, "mode": stat.S_IMODE(path.stat().st_mode)})
              elif stat.S_ISDIR(mode):
                  retained.append(name)
              else:
                  raise ValueError("dependency bundle contains a non-directory path")
          directories[:] = retained
          for name in sorted(names):
              path = root / name
              relative = path.relative_to(bundle).as_posix()
              mode = path.lstat().st_mode
              if path.is_symlink():
                  target = link_target(path)
                  links.append({"path": relative, "target": target, "mode": stat.S_IMODE(path.stat().st_mode)})
              elif stat.S_ISREG(mode):
                  files.append({"path": relative, "sha256": digest(path), "mode": stat.S_IMODE(mode)})
              else:
                  raise ValueError("dependency bundle contains a non-regular file")
    return {"files": files, "symlinks": links}


def copy_dependency_bundle(bundle: Path, destination: Path, expected: dict[str, list[dict[str, Any]]]) -> str:
    if destination.is_symlink() or not destination.is_dir():
        raise ValueError("dependency copy destination must be a real candidate directory")
    before = dependency_inventory(bundle)
    if before != expected:
        raise ValueError("dependency bundle inventory or hashes differ from receipt")
    if any((destination / name).exists() or (destination / name).is_symlink() for name in (".cache", "node_modules")):
        raise ValueError("candidate already contains a dependency directory")
    for row in before["files"]:
        source, target = bundle / row["path"], destination / row["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target, follow_symlinks=False)
        os.chmod(target, row["mode"])
    for row in before["symlinks"]:
        target = destination / row["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        os.symlink(row["target"], target)
    copied = dependency_inventory(destination, allow_other=True)
    if copied != before or dependency_inventory(bundle) != before:
        raise ValueError("dependency bundle drifted while copying")
    return hashlib.sha256(json.dumps(copied, separators=(",", ":"), sort_keys=True).encode()).hexdigest()


def dependency_fingerprint(inventory: dict[str, list[dict[str, Any]]]) -> str:
    return hashlib.sha256(json.dumps(inventory, separators=(",", ":"), sort_keys=True).encode()).hexdigest()


