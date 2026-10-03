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
MAX_EXECUTABLE_BYTES = 512 * 1024 * 1024
MAX_STDERR_BYTES = 4096
DEFAULT_TIMEOUT = 30.0
SCHEMA = "semaprax.token-comparison.v1"
SESSION_SCHEMA = "semaprax.token-comparison-session.v2"
MAX_SAFE_INTEGER = 9007199254740991


class ReportError(RuntimeError):
    pass


def checked_count(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and 0 <= value <= MAX_SAFE_INTEGER


def checked_add(left: int, right: int, label: str) -> int:
    if left > MAX_SAFE_INTEGER - right:
        raise ReportError(f"{label} exceeds the supported numeric range")
    return left + right


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


def executable_sha256(path: pathlib.Path) -> str:
    """Hash a compiler executable as bytes without treating it as text."""
    digest = hashlib.sha256()
    total = 0
    try:
        with path.open("rb") as handle:
            while chunk := handle.read(1024 * 1024):
                total += len(chunk)
                if total > MAX_EXECUTABLE_BYTES:
                    raise ReportError(f"compiler executable exceeds {MAX_EXECUTABLE_BYTES} bytes")
                digest.update(chunk)
    except OSError as error:
        raise ReportError(f"cannot read compiler executable: {error}") from error
    return "sha256:" + digest.hexdigest()


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
        "executable_sha256": executable_sha256(cli),
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


def write_rendered(path: pathlib.Path, rendered: str, overwrite: bool) -> None:
    """Write an explicitly requested human export without an overwrite race."""
    if path.exists() and not overwrite:
        raise ReportError(f"refusing to overwrite existing report: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: pathlib.Path | None = None
    try:
        with tempfile.NamedTemporaryFile(prefix=".token-summary-", dir=path.parent, delete=False) as handle:
            temporary = pathlib.Path(handle.name)
            handle.write(rendered.encode("utf-8"))
            handle.flush()
            os.fsync(handle.fileno())
        if overwrite:
            os.replace(temporary, path)
        else:
            os.link(temporary, path)
            temporary.unlink()
    except FileExistsError as error:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        raise ReportError(f"refusing to overwrite existing report: {path}") from error
    except OSError as error:
        if temporary is not None:
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
    method_rows: dict[tuple[tuple[Any, ...], str], dict[str, Any]] = {}
    observations: dict[tuple[Any, ...], list[dict[str, Any]]] = {}
    total = malformed = 0
    seen: dict[str, dict[str, Any]] = {}
    for number, line in enumerate(raw.splitlines(), 1):
        if not line:
            continue
        try:
            event = json.loads(line, object_pairs_hook=lambda pairs: dict(pairs) if len({key for key, _ in pairs}) == len(pairs) else (_ for _ in ()).throw(ValueError("duplicate key")))
        except (ValueError, json.JSONDecodeError) as error:
            raise ReportError(f"event stream line {number} is not JSON") from error
        required = ("schema", "eventId", "sessionId", "attemptSequence", "deliverySequence", "method", "boundary", "subjectRevision", "outcome", "status", "bytes", "digest", "tokenizer", "tokenizerFingerprint", "tokens", "referenceKind", "baselineTokens")
        if not isinstance(event, dict) or any(field not in event for field in required):
            malformed += 1
            continue
        if event["schema"] != "semaprax.token-observation.v1":
            malformed += 1
            continue
        identifiers = ("eventId", "sessionId", "method", "boundary")
        nullable_text = ("subjectRevision", "digest", "tokenizer", "tokenizerFingerprint", "referenceKind")
        nullable_count = ("bytes", "tokens", "baselineTokens")
        if any(not isinstance(event[key], str) or not event[key] or len(event[key].encode("utf-8")) > 4096 for key in identifiers) or len(event["method"].encode("utf-8")) > 128 or any(ord(char) < 32 or ord(char) == 127 for char in event["method"]) or any(event[key] is not None and (not isinstance(event[key], str) or len(event[key].encode("utf-8")) > 4096) for key in nullable_text) or any(not checked_count(event[key]) for key in ("attemptSequence", "deliverySequence")) or any(event[key] is not None and not checked_count(event[key]) for key in nullable_count) or event["outcome"] not in ("success", "error", "malformed", "timeout", "incomplete") or event["status"] not in ("measured", "tokenizer_unavailable", "tokenizer_failed", "baseline_unavailable", "incomplete"):
            malformed += 1
            continue
        previous = seen.get(event["eventId"])
        if previous is not None:
            if previous != event:
                raise ReportError(f"event stream repeats eventId with conflicting metadata: {event['eventId']}")
            continue
        seen[event["eventId"]] = event
        total += 1
        key = (
            event.get("tokenizer"), event.get("tokenizerFingerprint"), event["boundary"],
            event.get("referenceKind"),
        )
        group = groups.setdefault(key, {
            "tokenizer": key[0], "tokenizer_fingerprint": key[1], "boundary": key[2],
            "reference_kind": key[3], "coverage": {"events": 0, "token_measured": 0, "baseline_available": 0, "paired": 0},
            "outcomes": {}, "statuses": {}, "bytes": 0, "tokens": 0, "baseline_tokens": 0,
            "paired_actual_tokens": 0, "paired_baseline_tokens": 0,
        })
        group["coverage"]["events"] += 1
        method = event["method"]
        method_group = method_rows.setdefault((key, method), {
            "method": method,
            "events": 0,
            "paired": 0,
            "paired_actual_tokens": 0,
            "paired_baseline_tokens": 0,
        })
        method_group["events"] += 1
        group["outcomes"][str(event["outcome"])] = group["outcomes"].get(str(event["outcome"]), 0) + 1
        group["statuses"][str(event["status"])] = group["statuses"].get(str(event["status"]), 0) + 1
        if isinstance(event["bytes"], int) and not isinstance(event["bytes"], bool) and event["bytes"] >= 0:
            group["bytes"] = checked_add(group["bytes"], event["bytes"], "observed bytes")
        actual_measured = isinstance(event.get("tokens"), int) and not isinstance(event["tokens"], bool) and event["tokens"] >= 0
        baseline_available = isinstance(event.get("baselineTokens"), int) and not isinstance(event["baselineTokens"], bool) and event["baselineTokens"] >= 0
        if actual_measured:
            group["coverage"]["token_measured"] += 1
            group["tokens"] = checked_add(group["tokens"], event["tokens"], "observed tokens")
        if baseline_available:
            group["coverage"]["baseline_available"] += 1
            group["baseline_tokens"] = checked_add(group["baseline_tokens"], event["baselineTokens"], "observed baseline tokens")
        if actual_measured and baseline_available and event["outcome"] == "success" and event["status"] == "measured":
            group["coverage"]["paired"] += 1
            group["paired_actual_tokens"] = checked_add(group["paired_actual_tokens"], event["tokens"], "paired actual tokens")
            group["paired_baseline_tokens"] = checked_add(group["paired_baseline_tokens"], event["baselineTokens"], "paired baseline tokens")
            method_group["paired"] += 1
            method_group["paired_actual_tokens"] = checked_add(method_group["paired_actual_tokens"], event["tokens"], "method paired actual tokens")
            method_group["paired_baseline_tokens"] = checked_add(method_group["paired_baseline_tokens"], event["baselineTokens"], "method paired baseline tokens")
            observations.setdefault(key, []).append({"method": method, "delta_tokens": event["baselineTokens"] - event["tokens"]})
    ordered = [groups[key] for key in sorted(groups, key=lambda key: tuple("" if value is None else str(value) for value in key))]
    for group in ordered:
        key = (group["tokenizer"], group["tokenizer_fingerprint"], group["boundary"], group["reference_kind"])
        group["methods"] = [method_rows[(key, method)] for method in sorted(m for group_key, m in method_rows if group_key == key)]
        rows = observations.get(key, [])
        group["largest_reductions"] = sorted(
            (row for row in rows if row["delta_tokens"] > 0),
            key=lambda row: (-row["delta_tokens"], row["method"]),
        )[:3]
        group["largest_regressions"] = sorted(
            (row for row in rows if row["delta_tokens"] < 0),
            key=lambda row: (row["delta_tokens"], row["method"]),
        )[:3]
    identity_fields = {"report_kind": "session", "event_stream_sha256": sha256(raw), "groups": ordered, "malformed_events": malformed}
    return {"schema": SESSION_SCHEMA, "comparison_identity": identity(identity_fields), **identity_fields, "events": total}


def strict_json(data: bytes, label: str) -> dict[str, Any]:
    def unique_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ReportError(f"{label} has duplicate JSON key `{key}`")
            result[key] = value
        return result
    try:
        value = json.loads(data, object_pairs_hook=unique_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReportError(f"{label} is not valid JSON") from error
    if not isinstance(value, dict):
        raise ReportError(f"{label} must be a JSON object")
    return value


def show_counts(counts: Any) -> list[str]:
    if not isinstance(counts, dict) or counts.get("measurement_status") != "measured":
        return ["Model tokens unavailable; byte measurements remain separate."]
    baseline, actual, delta = counts.get("baseline_tokens"), counts.get("actual_tokens"), counts.get("delta_tokens")
    if not all(isinstance(value, int) and not isinstance(value, bool) and value >= 0 for value in (baseline, actual)) or not isinstance(delta, int) or delta != baseline - actual:
        raise ReportError("token report has invalid measured token counts")
    result = [f"Baseline tokens: {baseline}", f"Actual payload tokens: {actual}"]
    result.append(f"{delta} tokens saved versus reference." if delta > 0 else f"+{-delta} tokens used versus reference." if delta < 0 else "No token difference versus reference.")
    percentage = counts.get("delta_percentage")
    if percentage is not None:
        if not isinstance(percentage, (int, float)) or isinstance(percentage, bool):
            raise ReportError("token report has invalid percentage")
        result.append(f"Reported percentage: {percentage}%")
    return result


def show_text(value: dict[str, Any]) -> str:
    schema = value.get("schema")
    if schema == SCHEMA:
        if value.get("report_kind") not in ("projection", "compare") or not isinstance(value.get("baseline"), dict) or not isinstance(value.get("actual"), dict):
            raise ReportError("unsupported token comparison report")
        baseline, actual = value["baseline"], value["actual"]
        if not all(isinstance(row.get("utf8_bytes"), int) and not isinstance(row.get("utf8_bytes"), bool) and row["utf8_bytes"] >= 0 for row in (baseline, actual)):
            raise ReportError("token report has invalid byte counts")
        kind = value.get("baseline_kind") if value["report_kind"] == "projection" else value.get("reference_kind")
        if not isinstance(kind, str):
            raise ReportError("token report has invalid comparison type")
        lines = ["SEMAPRAX token report snapshot", "Current revision not verified. This local report is not live monitoring or a billed counter.", "", f"Comparison type: {kind}"]
        if value["report_kind"] == "projection":
            if not isinstance(value.get("source_revision"), str) or not isinstance(value.get("actual_kind"), str):
                raise ReportError("projection report lacks revision or boundary")
            lines.extend([f"Subject revision: {value['source_revision']}", f"Measured boundary: {value['actual_kind']}"])
        tokenizer = value.get("tokenizer")
        if isinstance(tokenizer, dict):
            lines.append(f"Tokenizer: {tokenizer.get('name', 'unavailable')}")
            fingerprint = tokenizer.get("vocabulary_fingerprint", tokenizer.get("fingerprint"))
            if isinstance(fingerprint, str):
                lines.append(f"Tokenizer fingerprint: {fingerprint}")
        else:
            lines.append("Tokenizer: unavailable")
        lines.extend([f"Baseline payload bytes: {baseline['utf8_bytes']}", f"Actual payload bytes: {actual['utf8_bytes']}"])
        lines.extend(show_counts(value.get("counts")))
        lines.extend(["", "Provider usage is not present in this report."])
        return "\n".join(lines) + "\n"
    if schema in ("semaprax.token-comparison-session.v1", SESSION_SCHEMA):
        groups = value.get("groups")
        if value.get("report_kind") != "session" or not isinstance(value.get("events"), int) or not isinstance(groups, list):
            raise ReportError("unsupported token session report")
        lines = ["SEMAPRAX session token report snapshot", "Current revision not verified. This local report is not live monitoring or a billed counter.", "", f"Observed events: {value['events']}", f"Malformed events excluded: {value.get('malformed_events', 0)}", "", "Grouped measurements"]
        for position, group in enumerate(groups, 1):
            if not isinstance(group, dict) or not isinstance(group.get("coverage"), dict):
                raise ReportError("session report has invalid group")
            coverage = group["coverage"]
            has_pair_coverage = "paired" in coverage
            coverage_fields = ("events", "paired") if has_pair_coverage else ("events", "token_measured", "baseline_available")
            if not all(isinstance(coverage.get(key), int) and not isinstance(coverage[key], bool) and coverage[key] >= 0 for key in coverage_fields):
                raise ReportError("session report has invalid coverage")
            pair_coverage = f"{coverage['paired']}/{coverage['events']} responses" if has_pair_coverage else f"unavailable in this v1 snapshot ({coverage['events']} responses)"
            lines.extend(["", f"Group {position}", f"Tokenizer: {group.get('tokenizer') if group.get('tokenizer') is not None else 'model tokens unavailable'}", f"Measured boundary: {group.get('boundary') if group.get('boundary') is not None else 'unavailable'}", f"Comparison type: {group.get('reference_kind') if group.get('reference_kind') is not None else 'unavailable'}", f"Measured pairs: {pair_coverage}"])
            fingerprint = group.get("tokenizer_fingerprint")
            lines.append(f"Tokenizer fingerprint: {fingerprint if isinstance(fingerprint, str) else 'unavailable'}")
            token_measured = coverage.get("token_measured")
            if not checked_count(token_measured):
                raise ReportError("session report has invalid token coverage")
            outcomes, statuses = group.get("outcomes"), group.get("statuses")
            if not isinstance(outcomes, dict) or not isinstance(statuses, dict) or any(not isinstance(key, str) or not checked_count(item) for table in (outcomes, statuses) for key, item in table.items()):
                raise ReportError("session report has invalid outcome/status counts")
            lines.extend([f"Token-measured observations: {token_measured}/{coverage['events']}", f"Unpaired observations: {coverage['events'] - coverage['paired']}" if has_pair_coverage else "Unpaired observations: unavailable in this v1 snapshot", "Outcome counts: " + ", ".join(f"{key}={outcomes[key]}" for key in sorted(outcomes)) if outcomes else "Outcome counts: none", "Status counts: " + ", ".join(f"{key}={statuses[key]}" for key in sorted(statuses)) if statuses else "Status counts: none"])
            if has_pair_coverage and coverage["paired"] != coverage["events"]:
                lines.append("Partial group: only paired successful measurements contribute to its reduction.")
            paired_actual, paired_baseline = group.get("paired_actual_tokens"), group.get("paired_baseline_tokens")
            if not has_pair_coverage:
                lines.append("Paired token reduction unavailable; this v1 snapshot has no paired totals.")
            elif group.get("tokenizer") is None or coverage["paired"] == 0:
                lines.append("Paired token reduction unavailable for this group.")
            elif not all(isinstance(item, int) and not isinstance(item, bool) and item >= 0 for item in (paired_actual, paired_baseline)):
                raise ReportError("session report has invalid paired token totals")
            else:
                delta = paired_baseline - paired_actual
                lines.extend([f"Paired actual payload tokens: {paired_actual}", f"Paired reference tokens: {paired_baseline}", f"{delta} tokens saved versus reference." if delta > 0 else f"+{-delta} tokens used versus reference." if delta < 0 else "No token difference versus reference."])
            if schema == SESSION_SCHEMA:
                methods = group.get("methods")
                if not isinstance(methods, list):
                    raise ReportError("session report has invalid method totals")
                lines.append("Method totals:")
                if not methods:
                    lines.append("  none")
                for method in methods:
                    if not isinstance(method, dict) or not isinstance(method.get("method"), str):
                        raise ReportError("session report has invalid method total")
                    lines.append(f"  {method['method']}: {method.get('paired_actual_tokens')} actual / {method.get('paired_baseline_tokens')} reference tokens; {method.get('paired')}/{method.get('events')} paired")
                for label, key in (("Largest reductions", "largest_reductions"), ("Largest regressions", "largest_regressions")):
                    rows = group.get(key)
                    if not isinstance(rows, list):
                        raise ReportError("session report has invalid extrema")
                    lines.append(label + ":")
                    if not rows:
                        lines.append("  none")
                    for row in rows:
                        if not isinstance(row, dict) or not isinstance(row.get("method"), str) or not isinstance(row.get("delta_tokens"), int):
                            raise ReportError("session report has invalid extreme")
                        change = f"{row['delta_tokens']} tokens saved" if row["delta_tokens"] > 0 else f"+{-row['delta_tokens']} tokens used"
                        lines.append(f"  {row['method']}: {change}")
        lines.extend(["", "Provider usage is not present in this report."])
        return "\n".join(lines) + "\n"
    raise ReportError("unsupported token report schema")


def markdown(text: str) -> str:
    # The renderer is deliberately a fenced text alternative: report strings
    # cannot become links, HTML, or Markdown instructions.
    return "# SEMAPRAX token report snapshot\n\n```text\n" + text.rstrip("\n") + "\n```\n"


def show(args: argparse.Namespace) -> str:
    value = strict_json(bounded_read(args.report, "token report"), "token report")
    rendered = show_text(value)
    return markdown(rendered) if args.format == "markdown" else rendered


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
    show_parser = commands.add_parser("show")
    show_parser.add_argument("report", type=pathlib.Path)
    show_parser.add_argument("--format", choices=("text", "markdown"), default="text")
    show_parser.add_argument("--output", type=pathlib.Path)
    show_parser.add_argument("--overwrite", action="store_true")
    return result


def main() -> int:
    args = parser().parse_args()
    if args.command == "show":
        rendered = show(args)
        if args.output is None:
            sys.stdout.write(rendered)
        else:
            write_rendered(args.output, rendered, args.overwrite)
        return 0
    document = projection(args) if args.command == "projection" else compare(args) if args.command == "compare" else session(args)
    write_report(args.output, document, args.overwrite)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReportError as error:
        print(f"token report error: {error}", file=sys.stderr)
        raise SystemExit(2)
