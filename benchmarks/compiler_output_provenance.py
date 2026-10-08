"""Validate runner-captured raw compiler outputs without executing candidate code."""
from __future__ import annotations
import hashlib
import json
from pathlib import Path
from typing import Any

SCHEMA = "semaprax.compiler-output-provenance.v1"

def digest(path: Path) -> str:
    if path.is_symlink() or not path.is_file(): raise ValueError(f"expected regular file: {path}")
    return hashlib.sha256(path.read_bytes()).hexdigest()

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
        if raw in raw_seen or final in final_seen: raise ValueError("compiler receipt output mapping is not one-to-one")
        raw_seen.add(raw); final_seen.add(final); target=_regular_under(raw_root, raw)
        try: target.resolve(strict=True).relative_to(raw_root)
        except ValueError as error: raise ValueError("compiler raw output escapes runner evidence") from error
        if target.is_symlink() or not target.is_file() or digest(target)!=sha: raise ValueError("compiler raw output differs from retained evidence")
        result[final]=sha
    return result
