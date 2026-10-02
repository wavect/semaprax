#!/usr/bin/env python3
"""Benchmark compact graph projections against full canonical graph output.

This is measurement tooling only. It invokes an explicitly selected Semaprax
CLI, replays each emitted wire document, and records byte/token measurements;
it does not authorize, publish, or make billing decisions. Token counts come
only from locally cached ``tiktoken`` encodings and are reported as evidence.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile
from token_measurement import sha256, tokenize_all

MAX_OUTPUT = 64 * 1024 * 1024
CASES = (
    "examples/banking_ledger.spx",
    "examples/http_app_routing.spx",
    "examples/calculator-project/semaprax.toml",
    "examples/frame-payload-project/semaprax.toml",
)
TASK_CONTEXT_CASES = {
    "examples/banking_ledger.spx": "ledger.apply",
    "examples/http_app_routing.spx": "app.main",
}
def run_bounded(command: list[str], cwd: pathlib.Path, timeout: float) -> bytes:
    with tempfile.TemporaryFile() as output:
        try:
            process = subprocess.Popen(command, cwd=cwd, stdout=output, stderr=subprocess.PIPE)
        except OSError as error:
            raise RuntimeError(f"failed to start CLI: {error}") from error
        try:
            _, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            process.kill()
            process.communicate()
            raise RuntimeError(f"CLI timed out after {timeout:g}s: {' '.join(command)}") from error
        if process.returncode != 0:
            detail = stderr.decode("utf-8", "replace")[:4096]
            raise RuntimeError(f"CLI failed ({process.returncode}): {detail}")
        output.seek(0, os.SEEK_END)
        size = output.tell()
        if size > MAX_OUTPUT:
            raise RuntimeError(f"CLI output exceeded {MAX_OUTPUT} bytes")
        output.seek(0)
        return output.read()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semaprax", required=True, type=pathlib.Path, help="explicit Semaprax CLI executable")
    parser.add_argument("--root", required=True, type=pathlib.Path, help="repository root containing benchmark cases")
    parser.add_argument("--output", required=True, type=pathlib.Path, help="result JSON path")
    parser.add_argument("--timeout", type=float, default=30.0, help="per CLI subprocess timeout in seconds")
    args = parser.parse_args()
    cli = args.semaprax.resolve()
    root = args.root.resolve()
    if not cli.is_file() or not os.access(cli, os.X_OK):
        parser.error(f"--semaprax is not an executable file: {cli}")
    if not root.is_dir():
        parser.error(f"--root is not a directory: {root}")
    rows = []
    for relative in CASES:
        case = root / relative
        if not case.is_file():
            raise RuntimeError(f"benchmark case is missing: {case}")
        case_rows = []
        payloads = {}
        full = None
        for encoding in ("text", "binary", "model-text"):
            encoded = run_bounded([str(cli), "compact", "graph", str(case), "--encoding", encoding], root, args.timeout)
            with tempfile.NamedTemporaryFile(prefix="semaprax-compact-", suffix=".wire", delete=False) as wire:
                wire_path = pathlib.Path(wire.name)
                wire.write(encoded)
            try:
                replay = run_bounded([str(cli), "compact", "graph", str(case), "--encoding", encoding, "--replay", str(wire_path)], root, args.timeout)
            finally:
                wire_path.unlink(missing_ok=True)
            if full is not None and full != replay:
                raise RuntimeError(f"wire replays disagree for {relative}")
            full = replay
            case_rows.append({"encoding": encoding, "compact": {"sha256": sha256(encoded), "bytes": len(encoded)}, "replay": {"sha256": sha256(replay), "bytes": len(replay)}})
            if encoding != "binary":
                payloads["compact_" + encoding.replace("-", "_")] = encoded
        ordinary = run_bounded([str(cli), "graph", str(case)], root, args.timeout)
        # The ordinary graph CLI adds one display LF after the canonical report.
        if ordinary != full and ordinary != full + b"\n":
            raise RuntimeError(f"compact replay differs from ordinary graph for {relative}")
        payloads["full"] = full
        token_data = tokenize_all(payloads)
        for tokenizer in token_data.values():
            full_tokens = tokenizer["tokens"]["full"]
            tokenizer["token_ratios"] = {
                label: (count / full_tokens if full_tokens else None)
                for label, count in tokenizer["tokens"].items()
                if label != "full"
            }
        row = {"case": relative, "full": {"sha256": sha256(full), "bytes": len(full)}, "projections": case_rows, "tokenizers": token_data}
        if relative in TASK_CONTEXT_CASES:
            stable_id = TASK_CONTEXT_CASES[relative]
            context_rows = []
            context_payloads = {}
            context_full = None
            for encoding in ("text", "binary", "model-text"):
                command = [str(cli), "compact", "task-context", str(case), stable_id, "--encoding", encoding]
                encoded = run_bounded(command, root, args.timeout)
                with tempfile.NamedTemporaryFile(prefix="semaprax-task-context-", suffix=".wire", delete=False) as wire:
                    wire_path = pathlib.Path(wire.name)
                    wire.write(encoded)
                try:
                    replay = run_bounded(command + ["--replay", str(wire_path)], root, args.timeout)
                finally:
                    wire_path.unlink(missing_ok=True)
                if context_full is not None and context_full != replay:
                    raise RuntimeError(f"task-context wire replays disagree for {relative} ({stable_id})")
                context_full = replay
                context_rows.append({"encoding": encoding, "compact": {"sha256": sha256(encoded), "bytes": len(encoded)}, "replay": {"sha256": sha256(replay), "bytes": len(replay)}})
                if encoding != "binary":
                    context_payloads["compact_" + encoding.replace("-", "_")] = encoded
            context_payloads["full"] = context_full
            context_tokens = tokenize_all(context_payloads)
            for tokenizer in context_tokens.values():
                full_tokens = tokenizer["tokens"]["full"]
                tokenizer["token_ratios"] = {
                    label: (count / full_tokens if full_tokens else None)
                    for label, count in tokenizer["tokens"].items() if label != "full"
                }
            for tokenizer in context_tokens.values():
                full_tokens = tokenizer["tokens"]["full"]
                tokenizer["token_ratios"] = {label: (count / full_tokens if full_tokens else None) for label, count in tokenizer["tokens"].items() if label != "full"}
            row["task_context"] = {"stable_id": stable_id, "full": {"sha256": sha256(context_full), "bytes": len(context_full)}, "projections": context_rows, "tokenizers": context_tokens}
        rows.append(row)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({"schema": "semaprax.compact-projection-benchmark.v1", "cases": rows}, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as error:
        print(f"benchmark error: {error}", file=sys.stderr)
        raise SystemExit(2)
