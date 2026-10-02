#!/usr/bin/env python3
"""Write an offline, exact-byte token comparison for compact projections.

This helper measures locally cached tokenizer assets.  It does not report
billable tokens, money, prompts, source text, raw compiler payloads, or paths.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
from typing import Any

from token_measurement import SUPPORTED_ENCODINGS, TokenizerUnavailable, measure_utf8, sha256


MAX_TEXT_BYTES = 64 * 1024 * 1024
MAX_STDERR_BYTES = 4096
DEFAULT_TIMEOUT = 30.0
SCHEMA = "semaprax.token-comparison.v1"


class ReportError(RuntimeError):
    pass


def bounded_read(path: pathlib.Path, label: str) -> bytes:
    try:
        with path.open("rb") as handle:
            data = handle.read(MAX_TEXT_BYTES + 1)
    except OSError as error:
        raise ReportError(f"cannot read {label}: {error}") from error
    if len(data) > MAX_TEXT_BYTES:
        raise ReportError(f"{label} exceeds {MAX_TEXT_BYTES} bytes")
    try:
        data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReportError(f"{label} must be UTF-8") from error
    return data


def run_bounded(command: list[str], cwd: pathlib.Path, timeout: float) -> bytes:
    with tempfile.TemporaryFile() as output:
        try:
            process = subprocess.Popen(
                command, cwd=cwd, stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.PIPE
            )
        except OSError as error:
            raise ReportError(f"failed to start compiler: {error}") from error
        try:
            _, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            process.kill()
            process.communicate()
            raise ReportError(f"compiler timed out after {timeout:g}s") from error
        if process.returncode != 0:
            detail = stderr[:MAX_STDERR_BYTES].decode("utf-8", "replace")
            raise ReportError(f"compiler failed ({process.returncode}): {detail}")
        output.seek(0, os.SEEK_END)
        size = output.tell()
        if size > MAX_TEXT_BYTES:
            raise ReportError(f"compiler output exceeds {MAX_TEXT_BYTES} bytes")
        output.seek(0)
        return output.read()


def framed_metadata(payload: bytes) -> dict[str, str]:
    """Read the common compact text/model-text metadata without retaining body."""
    try:
        newline = payload.index(b"\n")
    except ValueError as error:
        raise ReportError("projection envelope lacks a header line") from error
    if not payload[:newline].startswith(b"SEMAPRAX-"):
        raise ReportError("projection envelope has an unexpected header")
    cursor = newline + 1
    fields: dict[str, str] = {}
    for expected in (b"profile", b"root", b"source_revision"):
        prefix = expected + b" "
        if not payload.startswith(prefix, cursor):
            raise ReportError(f"projection envelope is missing {expected.decode('ascii')}")
        length_start = cursor + len(prefix)
        length_end = payload.find(b" ", length_start)
        if length_end < 0:
            raise ReportError("projection envelope has truncated metadata")
        length_text = payload[length_start:length_end]
        if not length_text.isdigit() or (len(length_text) > 1 and length_text.startswith(b"0")):
            raise ReportError("projection envelope has a noncanonical metadata length")
        length = int(length_text)
        value_start = length_end + 1
        value_end = value_start + length
        if value_end >= len(payload) or payload[value_end:value_end + 1] != b"\n":
            raise ReportError("projection envelope has truncated metadata value")
        try:
            fields[expected.decode("ascii")] = payload[value_start:value_end].decode("utf-8")
        except UnicodeDecodeError as error:
            raise ReportError("projection envelope metadata must be UTF-8") from error
        cursor = value_end + 1
    return fields


def comparison_counts(baseline: bytes, actual: bytes, tokenizer: str, allow_bytes_only: bool) -> tuple[dict[str, Any], dict[str, Any] | None]:
    try:
        baseline_tokens, metadata = measure_utf8(baseline, tokenizer)
        actual_tokens, actual_metadata = measure_utf8(actual, tokenizer)
    except TokenizerUnavailable as error:
        if not allow_bytes_only:
            raise ReportError(str(error)) from error
        return {
            "measurement_status": "tokenizer_unavailable",
            "baseline_tokens": None,
            "actual_tokens": None,
            "delta_tokens": None,
            "delta_fraction": None,
            "delta_percentage": None,
        }, None
    if metadata != actual_metadata:
        raise ReportError("tokenizer metadata changed during one comparison")
    delta = baseline_tokens - actual_tokens
    fraction = None if baseline_tokens == 0 else {"numerator": delta, "denominator": baseline_tokens}
    return {
        "measurement_status": "measured",
        "baseline_tokens": baseline_tokens,
        "actual_tokens": actual_tokens,
        "delta_tokens": delta,
        "delta_fraction": fraction,
        "delta_percentage": None if fraction is None else (100.0 * delta / baseline_tokens),
    }, metadata


def bytes_fact(data: bytes) -> dict[str, Any]:
    return {"sha256": sha256(data), "utf8_bytes": len(data)}


def identity(document: dict[str, Any]) -> str:
    encoded = json.dumps(document, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    return sha256(encoded)


def confidential_binding(value: Any) -> str:
    """Bind names, IDs, and option values without copying them into reports."""
    return identity({"value": value})


def validate_producer_options(profile: str, options: list[list[str]]) -> None:
    admitted = {
        "graph": set(),
        "context": {"--max-bytes"},
        "task-context": {
            "--seed", "--priority", "--reason", "--goal", "--revision",
            "--tokenizer", "--max-bytes", "--max-tokens",
        },
    }[profile]
    for option, value in options:
        if option not in admitted:
            raise ReportError(f"producer option `{option}` is not admitted for {profile}")
        if not value or value.startswith("-"):
            raise ReportError("producer option value must be nonempty and explicit")


def executable_fact(cli: pathlib.Path, root: pathlib.Path, timeout: float) -> dict[str, Any]:
    version = run_bounded([str(cli), "version", "--json"], root, timeout)
    try:
        parsed = json.loads(version)
    except json.JSONDecodeError as error:
        raise ReportError("compiler version output is not JSON") from error
    if not isinstance(parsed, dict) or not isinstance(parsed.get("version"), str):
        raise ReportError("compiler version output lacks a version")
    commit = parsed.get("commit")
    if not isinstance(commit, str):
        commit = None
    return {
        "version": parsed["version"],
        "commit": commit,
        "executable_sha256": sha256(bounded_read(cli, "compiler executable")),
    }


def write_report(path: pathlib.Path, document: dict[str, Any], overwrite: bool) -> None:
    if path.exists() and not overwrite:
        raise ReportError(f"refusing to overwrite existing report: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = (json.dumps(document, sort_keys=True, indent=2) + "\n").encode("utf-8")
    try:
        with tempfile.NamedTemporaryFile(prefix=".token-report-", dir=path.parent, delete=False) as handle:
            temporary = pathlib.Path(handle.name)
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        if overwrite:
            os.replace(temporary, path)
        else:
            # link(2) has create-if-absent semantics, unlike a second exists
            # check followed by replace, so another writer cannot lose a race.
            os.link(temporary, path)
            temporary.unlink()
    except FileExistsError as error:
        temporary.unlink(missing_ok=True)
        raise ReportError(f"refusing to overwrite existing report: {path}") from error
    except OSError as error:
        temporary.unlink(missing_ok=True)
        raise ReportError(f"cannot write report: {error}") from error


def projection(args: argparse.Namespace) -> dict[str, Any]:
    cli = args.semaprax.resolve()
    root = pathlib.Path.cwd().resolve()
    input_path = args.input.resolve()
    if not cli.is_file() or not os.access(cli, os.X_OK):
        raise ReportError("--semaprax must name an executable file")
    if not input_path.is_file():
        raise ReportError("--input must name a regular file")
    if args.profile not in ("graph", "context", "task-context"):
        raise ReportError("--profile must be graph, context, or task-context")
    if args.profile in ("context", "task-context") and not args.selection:
        raise ReportError(f"{args.profile} requires --selection")
    if args.profile == "graph" and args.selection:
        raise ReportError("graph does not accept --selection")
    validate_producer_options(args.profile, args.producer_option)
    before = bounded_read(input_path, "projection input")
    producer = [str(cli), "compact", args.profile, str(input_path)]
    if args.selection:
        producer.append(args.selection)
    producer.extend(["--encoding", args.encoding])
    for option in args.producer_option:
        producer.extend(option)
    actual = run_bounded(producer, root, args.timeout)
    metadata = framed_metadata(actual)
    with tempfile.NamedTemporaryFile(prefix="semaprax-token-replay-", suffix=".wire", delete=False) as handle:
        replay_path = pathlib.Path(handle.name)
        handle.write(actual)
    try:
        baseline = run_bounded(producer + ["--replay", str(replay_path)], root, args.timeout)
    finally:
        replay_path.unlink(missing_ok=True)
    after = bounded_read(input_path, "projection input")
    # The compiler replay authenticates selected content. Re-running the exact
    # producer also catches a Project manifest's transitive revision changing.
    replay_actual = run_bounded(producer, root, args.timeout)
    if before != after or actual != replay_actual:
        raise ReportError("projection input or selected revision changed during comparison")
    counts, tokenizer_metadata = comparison_counts(baseline, actual, args.measurement_tokenizer, args.allow_bytes_only)
    identity_fields = {
        "report_kind": "projection",
        "profile": metadata["profile"],
        "root_sha256": confidential_binding(metadata["root"]),
        "selection_sha256": confidential_binding(args.selection),
        "source_revision": metadata["source_revision"],
        "producer_options_sha256": confidential_binding(args.producer_option),
        "baseline": bytes_fact(baseline),
        "actual": bytes_fact(actual),
        "tokenizer": tokenizer_metadata,
        "counts": counts,
    }
    document = {
        "schema": SCHEMA,
        "comparison_identity": identity(identity_fields),
        **identity_fields,
        "baseline_kind": "same_selected_json",
        "actual_kind": f"compact_{args.encoding}",
        "display_lf_in_measurement": False,
        "compiler": executable_fact(cli, root, args.timeout),
    }
    return document


def compare(args: argparse.Namespace) -> dict[str, Any]:
    baseline = bounded_read(args.baseline, "baseline")
    actual = bounded_read(args.actual, "actual")
    counts, tokenizer_metadata = comparison_counts(baseline, actual, args.measurement_tokenizer, args.allow_bytes_only)
    identity_fields = {
        "report_kind": "compare",
        "reference_kind": args.reference_kind,
        "equivalence": "reference_only_not_verified",
        "baseline": bytes_fact(baseline),
        "actual": bytes_fact(actual),
        "tokenizer": tokenizer_metadata,
        "counts": counts,
    }
    return {"schema": SCHEMA, "comparison_identity": identity(identity_fields), **identity_fields}


def session(args: argparse.Namespace) -> dict[str, Any]:
    """Aggregate metadata-only #356 observations without retaining events."""
    raw = bounded_read(args.events, "event stream")
    groups: dict[tuple[Any, ...], dict[str, Any]] = {}
    total = malformed = 0
    for number, line in enumerate(raw.splitlines(), 1):
        if not line:
            continue
        total += 1
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ReportError(f"event stream line {number} is not JSON") from error
        required = ("schema", "boundary", "outcome", "status", "bytes", "digest")
        if not isinstance(event, dict) or any(field not in event for field in required):
            malformed += 1
            continue
        if event["schema"] != "semaprax.token-observation.v1":
            malformed += 1
            continue
        key = (
            event.get("tokenizer"), event.get("tokenizerFingerprint"), event["boundary"],
            event.get("referenceKind"),
        )
        group = groups.setdefault(key, {
            "tokenizer": key[0], "tokenizer_fingerprint": key[1], "boundary": key[2],
            "reference_kind": key[3], "coverage": {"events": 0, "token_measured": 0, "baseline_available": 0},
            "outcomes": {}, "statuses": {}, "bytes": 0, "tokens": 0, "baseline_tokens": 0,
        })
        group["coverage"]["events"] += 1
        group["outcomes"][str(event["outcome"])] = group["outcomes"].get(str(event["outcome"]), 0) + 1
        group["statuses"][str(event["status"])] = group["statuses"].get(str(event["status"]), 0) + 1
        if isinstance(event["bytes"], int) and not isinstance(event["bytes"], bool) and event["bytes"] >= 0:
            group["bytes"] += event["bytes"]
        if isinstance(event.get("tokens"), int) and not isinstance(event["tokens"], bool) and event["tokens"] >= 0:
            group["coverage"]["token_measured"] += 1
            group["tokens"] += event["tokens"]
        if isinstance(event.get("baselineTokens"), int) and not isinstance(event["baselineTokens"], bool) and event["baselineTokens"] >= 0:
            group["coverage"]["baseline_available"] += 1
            group["baseline_tokens"] += event["baselineTokens"]
    ordered = [groups[key] for key in sorted(groups, key=lambda key: tuple("" if value is None else str(value) for value in key))]
    identity_fields = {"report_kind": "session", "event_stream_sha256": sha256(raw), "groups": ordered, "malformed_events": malformed}
    return {"schema": "semaprax.token-comparison-session.v1", "comparison_identity": identity(identity_fields), **identity_fields, "events": total}


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--measurement-tokenizer", required=True, choices=SUPPORTED_ENCODINGS)
    common.add_argument("--allow-bytes-only", action="store_true")
    common.add_argument("--output", required=True, type=pathlib.Path)
    common.add_argument("--overwrite", action="store_true")
    projection_parser = commands.add_parser("projection", parents=[common])
    projection_parser.add_argument("--semaprax", required=True, type=pathlib.Path)
    projection_parser.add_argument("--input", required=True, type=pathlib.Path)
    projection_parser.add_argument("--profile", required=True)
    projection_parser.add_argument("--selection")
    projection_parser.add_argument("--encoding", required=True, choices=("text", "model-text"))
    projection_parser.add_argument("--producer-option", action="append", nargs=2, default=[], metavar=("OPTION", "VALUE"))
    projection_parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    compare_parser = commands.add_parser("compare", parents=[common])
    compare_parser.add_argument("--baseline", required=True, type=pathlib.Path)
    compare_parser.add_argument("--actual", required=True, type=pathlib.Path)
    compare_parser.add_argument("--reference-kind", required=True, choices=("source_context", "user_reference"))
    session_parser = commands.add_parser("session")
    session_parser.add_argument("--events", required=True, type=pathlib.Path)
    session_parser.add_argument("--output", required=True, type=pathlib.Path)
    session_parser.add_argument("--overwrite", action="store_true")
    return result


def main() -> int:
    args = parser().parse_args()
    document = projection(args) if args.command == "projection" else compare(args) if args.command == "compare" else session(args)
    write_report(args.output, document, args.overwrite)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReportError as error:
        print(f"token report error: {error}", file=sys.stderr)
        raise SystemExit(2)
