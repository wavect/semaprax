"""Validate runner-captured raw compiler outputs without executing candidate code."""
from __future__ import annotations
import hashlib
import json
import os
import shutil
import stat
from pathlib import Path
from typing import Any

SCHEMA = "semaprax.compiler-output-provenance.v1"
INPUT_SNAPSHOT_SCHEMA = "semaprax.compiler-input-snapshot.v1"
MAX_INPUT_RECEIPT_BYTES = 1024 * 1024


def _snapshot_read(root_fd: int, relative: str, limit: int) -> bytes:
    """Read only a regular file through no-follow, directory-relative handles."""
    parent_fd = os.dup(root_fd)
    try:
        parts = Path(relative).parts
        for component in parts[:-1]:
            next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
            os.close(parent_fd)
            parent_fd = next_fd
        fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent_fd)
        try:
            stream = os.fdopen(fd, "rb")
        except BaseException:
            os.close(fd)
            raise
        with stream:
            if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
                raise ValueError("snapshot input is not a regular file")
            return stream.read(limit + 1)
    except OSError as error:
        raise ValueError("snapshot input cannot be opened without following symlinks") from error
    finally:
        os.close(parent_fd)


def capture_inputs(candidate: Path, destination: Path, paths: list[str],
                   max_bytes: int = 8 * 1024 * 1024) -> tuple[Path, str]:
    """Retain pre-generation bytes in a new runner-owned evidence directory.

    The runner supplies the input closure. This does not establish its
    completeness, compiler execution, generated-file authorship, or billing.
    Run the compiler against the retained snapshot when exact input binding is
    required; a collection of independently read live files is not a lock.
    """
    if candidate.is_symlink() or not candidate.is_dir():
        raise ValueError("candidate must be a real directory")
    if destination.resolve().is_relative_to(candidate.resolve()):
        raise ValueError("input snapshot must be outside the candidate")
    if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or max_bytes < 0:
        raise ValueError("snapshot byte budget must be nonnegative")
    if not isinstance(paths, list) or not paths:
        raise ValueError("snapshot input selection must be nonempty")
    selected = [_relative(path, "snapshot input path") for path in paths]
    if len(set(selected)) != len(selected):
        raise ValueError("snapshot input selection has duplicate paths")
    if not all(hasattr(os, flag) for flag in ("O_DIRECTORY", "O_NOFOLLOW", "O_NONBLOCK")):
        raise ValueError("safe input snapshot capture requires no-follow directory handles")
    # Exclusive creation protects earlier retained evidence from replacement.
    destination.mkdir()
    try:
        inputs = destination / "inputs"
        inputs.mkdir()
        rows = []
        total = 0
        root_fd = os.open(candidate, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            for relative in sorted(selected):
                _regular_under(candidate, relative)
                data = _snapshot_read(root_fd, relative, max_bytes - total)
                if len(data) > max_bytes - total:
                    raise ValueError("snapshot input selection exceeds byte budget")
                target = inputs / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                with target.open("xb") as stream:
                    stream.write(data)
                total += len(data)
                rows.append({"path": relative, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)})
        finally:
            os.close(root_fd)
        receipt = destination / "snapshot.json"
        encoded = (json.dumps({"schema": INPUT_SNAPSHOT_SCHEMA,
            "input_root": "inputs", "input_files": rows, "total_bytes": total},
            sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
        if len(encoded) > MAX_INPUT_RECEIPT_BYTES:
            raise ValueError("input snapshot receipt exceeds byte budget")
        receipt.write_bytes(encoded)
        return receipt, digest(receipt)
    except BaseException:
        shutil.rmtree(destination)
        raise


def validate_input_snapshot(receipt: Path, expected_sha256: str) -> list[dict[str, Any]]:
    """Validate historical bytes in isolated, runner-owned retained evidence.

    The expected digest must come from immutable runner provenance outside the
    candidate. This validator assumes the evidence directory is isolated from
    concurrent writers; it does not grant a lock or freeze a mutable directory.
    """
    if receipt.is_symlink() or not receipt.is_file():
        raise ValueError("input snapshot receipt must be a regular file")
    with receipt.open("rb") as stream:
        encoded = stream.read(MAX_INPUT_RECEIPT_BYTES + 1)
    if len(encoded) > MAX_INPUT_RECEIPT_BYTES:
        raise ValueError("input snapshot receipt exceeds byte budget")
    if hashlib.sha256(encoded).hexdigest() != _hex(expected_sha256, 64, "expected input snapshot"):
        raise ValueError("input snapshot differs from immutable runner provenance")
    value = json.loads(encoded)
    if encoded != (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8"):
        raise ValueError("input snapshot receipt is not canonical")
    if (not isinstance(value, dict) or set(value) != {"schema", "input_root", "input_files", "total_bytes"}
            or value["schema"] != INPUT_SNAPSHOT_SCHEMA or value["input_root"] != "inputs"):
        raise ValueError("input snapshot schema differs")
    rows = value["input_files"]
    if not isinstance(rows, list) or not rows:
        raise ValueError("input snapshot selection differs")
    seen = set()
    total = 0
    previous = ""
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "bytes"}:
            raise ValueError("input snapshot row differs")
        relative = _relative(row["path"], "snapshot input path")
        if relative in seen:
            raise ValueError("input snapshot duplicates an input path")
        if relative < previous:
            raise ValueError("input snapshot input order is not canonical")
        previous = relative
        seen.add(relative)
        count = row["bytes"]
        target = _regular_under(receipt.parent, "inputs/" + relative)
        if (isinstance(count, bool) or not isinstance(count, int) or count < 0
                or target.stat().st_size != count or digest(target) != _hex(row["sha256"], 64, "snapshot input")):
            raise ValueError("input snapshot bytes differ from retained evidence")
        total += count
    if (isinstance(value["total_bytes"], bool) or not isinstance(value["total_bytes"], int)
            or value["total_bytes"] != total):
        raise ValueError("input snapshot total differs")
    return rows

def digest(path: Path) -> str:
    if path.is_symlink() or not path.is_file(): raise ValueError(f"expected regular file: {path}")
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            value.update(chunk)
    return value.hexdigest()

def _relative(value: Any, label: str) -> str:
    if not isinstance(value,str) or not value: raise ValueError(f"{label} must be a nonempty relative path")
    path=Path(value)
    if value == '.' or path.is_absolute() or ".." in path.parts or path.as_posix()!=value: raise ValueError(f"{label} must be a safe relative path")
    return value

def _regular_under(root: Path, relative: str) -> Path:
    target = root / relative
    for part in [target, *target.parents]:
        if part == root: break
        if part.is_symlink(): raise ValueError("compiler evidence has a symlink ancestor")
    try: target.resolve(strict=True).relative_to(root.resolve(strict=True))
    except ValueError as error: raise ValueError("compiler evidence escapes its root") from error
    if not target.is_file(): raise ValueError("compiler evidence is not a regular file")
    return target

def _hex(value: Any, width: int, label: str) -> str:
    if not isinstance(value,str) or len(value)!=width or any(c not in '0123456789abcdef' for c in value): raise ValueError(f"{label} has invalid hash")
    return value

def validate(receipt_path: Path, expected_receipt_sha256: str, candidate: Path,
             compiler_source_sha: str, compiler_binary_sha256: str) -> dict[str, str]:
    """Return final-path hashes backed by a trusted, retained raw-output receipt."""
    if receipt_path.is_symlink() or not receipt_path.is_file(): raise ValueError("compiler receipt must be a regular file")
    if digest(receipt_path)!=_hex(expected_receipt_sha256,64,"expected receipt"): raise ValueError("compiler receipt differs from immutable attempt provenance")
    receipt=json.loads(receipt_path.read_text(encoding='utf-8'))
    if not isinstance(receipt,dict) or set(receipt)!={"schema","compiler","cwd","argv","input_files","raw_root","raw_outputs","repeat_outputs"} or receipt.get("schema")!=SCHEMA: raise ValueError("compiler receipt schema differs")
    if receipt.get('cwd') != '.' or not isinstance(receipt.get('argv'),list): raise ValueError('compiler receipt invocation differs')
    argv = receipt['argv']
    if len(argv) not in (4, 6) or argv[0] != 'webapp' or argv[2:4] != ['-o', '{output}']:
        raise ValueError('compiler receipt invocation differs')
    source = _relative(argv[1], 'compiler source argument')
    if len(argv) == 6 and (argv[4] != '--title' or not isinstance(argv[5], str)):
        raise ValueError('compiler receipt title argument differs')
    compiler=receipt.get("compiler")
    if not isinstance(compiler,dict) or set(compiler)!={"source_sha","binary_sha256"}: raise ValueError("compiler receipt compiler identity differs")
    if (_hex(compiler['source_sha'],40,'compiler source')!=_hex(compiler_source_sha,40,'expected compiler source') or _hex(compiler['binary_sha256'],64,'compiler binary')!=_hex(compiler_binary_sha256,64,'expected compiler binary')): raise ValueError("compiler receipt does not bind the trusted compiler")
    if candidate.is_symlink() or not candidate.is_dir(): raise ValueError("candidate must be a real directory")
    inputs=receipt.get('input_files')
    if not isinstance(inputs,list) or not inputs: raise ValueError("compiler receipt input closure differs")
    seen=set()
    for row in inputs:
        if not isinstance(row,dict) or set(row)!={'path','sha256'}: raise ValueError("compiler receipt input row differs")
        rel=_relative(row['path'],'compiler input path'); sha=_hex(row['sha256'],64,'compiler input')
        if rel in seen: raise ValueError("compiler receipt duplicates an input path")
        seen.add(rel); target=_regular_under(candidate, rel)
        if target.is_symlink() or not target.is_file() or digest(target)!=sha: raise ValueError("compiler receipt input differs from retained candidate")
    if source not in seen: raise ValueError('compiler source argument is outside the input closure')
    raw_root_rel=_relative(receipt.get('raw_root'),'compiler raw root')
    raw_root_path=receipt_path.parent/raw_root_rel
    if raw_root_path.is_symlink() or not raw_root_path.is_dir(): raise ValueError("compiler raw root is unsafe")
    for part in [raw_root_path, *raw_root_path.parents]:
        if part == receipt_path.parent: break
        if part.is_symlink(): raise ValueError('compiler raw root has a symlink ancestor')
    raw_root=raw_root_path.resolve(strict=True)
    try: raw_root.relative_to(receipt_path.parent.resolve(strict=True))
    except ValueError as error: raise ValueError('compiler raw root escapes runner evidence') from error
    rows=receipt.get('raw_outputs')
    if not isinstance(rows,list) or not rows or not all(isinstance(row,dict) for row in rows): raise ValueError("compiler receipt raw outputs differ")
    if receipt.get('repeat_outputs') != [{"raw_path": row.get("raw_path"), "sha256": row.get("sha256")} for row in rows]: raise ValueError('compiler receipt repeat manifest differs')
    raw_seen=set(); final_seen=set(); result={}
    for row in rows:
        if not isinstance(row,dict) or set(row)!={'raw_path','final_path','sha256'}: raise ValueError("compiler receipt raw output row differs")
        raw=_relative(row['raw_path'],'raw output path'); final=_relative(row['final_path'],'final output path'); sha=_hex(row['sha256'],64,'raw output')
        # v1 proves whole-file generated outputs, not mixed source projections.
        # A compiler copying an input into its output cannot erase authorship.
        if final in seen: raise ValueError("compiler output overlaps authored input closure")
        if raw in raw_seen or final in final_seen: raise ValueError("compiler receipt output mapping is not one-to-one")
        raw_seen.add(raw); final_seen.add(final); target=_regular_under(raw_root, raw)
        try: target.resolve(strict=True).relative_to(raw_root)
        except ValueError as error: raise ValueError("compiler raw output escapes runner evidence") from error
        if target.is_symlink() or not target.is_file() or digest(target)!=sha: raise ValueError("compiler raw output differs from retained evidence")
        result[final]=sha
    return result
