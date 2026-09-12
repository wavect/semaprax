#!/usr/bin/env python3
"""Host-owned OpenCode availability smoke for a pinned, non-editing model.

This adapter is deliberately outside the comparative-pilot runner.  It can
capture an OpenCode JSON event stream for one frozen prompt, but it cannot
write a candidate, produce a ledger, report provider usage, or lift the
pilot's HUMAN_BLOCKED gate.  Authentication remains in OpenCode's own host
credential store: this program never accepts an API-key environment variable,
credential value, endpoint, or fallback model.
"""

import argparse
import hashlib
import json
import os
import selectors
import signal
import subprocess
import sys
import time
from pathlib import Path


SMOKE_AGENT = "semaprax-smoke"
DEFAULT_OPENCODE = "/opt/homebrew/bin/opencode"
DEFAULT_MODEL = "opencode/muse-spark-1.3-contributor-free"
MAX_TIMEOUT_SECONDS = 600
MAX_OUTPUT_BYTES = 1_048_576
MAX_PROMPT_BYTES = 65_536
MAX_SESSION_EXPORT_BYTES = 1_048_576


class SmokeFailure(Exception):
    pass


def sha256(body):
    return hashlib.sha256(body).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def positive(value, label, maximum):
    if not isinstance(value, int) or value <= 0 or value > maximum:
        raise SmokeFailure(f"{label} must be an integer from 1 through {maximum}")
    return value


def exact_model(value):
    if not isinstance(value, str) or not value or "/" not in value:
        raise SmokeFailure("--model must be an exact provider/model identifier")
    if any(character.isspace() for character in value):
        raise SmokeFailure("--model may not contain whitespace")
    return value


def load_frozen_prompt(path, expected_sha256):
    try:
        with Path(path).open("rb") as frozen:
            body = frozen.read(MAX_PROMPT_BYTES + 1)
    except OSError as error:
        raise SmokeFailure(f"unable to read frozen prompt: {error}") from error
    if len(body) > MAX_PROMPT_BYTES:
        raise SmokeFailure(f"frozen prompt exceeds {MAX_PROMPT_BYTES} bytes")
    if sha256(body) != expected_sha256:
        raise SmokeFailure("--prompt-sha256 does not match the frozen prompt bytes")
    try:
        return body.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SmokeFailure("frozen prompt must be UTF-8") from error


def policy_document():
    # OpenCode's documented agent-specific permission syntax.  `--pure` also
    # disables external plugins; neither option is presented as an OS sandbox.
    return {
        "$schema": "https://opencode.ai/config.json",
        "agent": {
            SMOKE_AGENT: {
                "description": "SEMAPRAX non-editing provider availability smoke",
                "permission": {"*": "deny"},
            }
        },
    }


def prepare_empty_sandbox(path):
    sandbox = Path(path).resolve(strict=True)
    if not sandbox.is_dir():
        raise SmokeFailure("--sandbox must name an existing directory")
    if any(sandbox.iterdir()):
        raise SmokeFailure("--sandbox must be empty so the smoke policy cannot override task files")
    policy_path = sandbox / "opencode.json"
    policy = json.dumps(policy_document(), sort_keys=True, separators=(",", ":")) + "\n"
    policy_path.write_text(policy, encoding="utf-8")
    return sandbox, policy_path, policy.encode("utf-8")


def output_path_outside_sandbox(path, sandbox):
    output = Path(path)
    if output.exists():
        raise SmokeFailure("--raw-events-output must not already exist")
    parent = output.parent.resolve(strict=True)
    output = parent / output.name
    try:
        output.relative_to(sandbox)
    except ValueError:
        return output
    raise SmokeFailure("--raw-events-output must be outside the smoke sandbox")


def command(opencode, model, sandbox, prompt):
    # `run --help` documents all of these flags.  There is intentionally no
    # `--auto`, no retry, and no alternate model/provider argument.
    return [
        str(opencode), "run", "--pure", "--agent", SMOKE_AGENT,
        "--model", model, "--format", "json", "--dir", str(sandbox), prompt,
    ]


def exact_text(value, label):
    if not isinstance(value, str) or not value:
        raise SmokeFailure(f"{label} must be nonempty text")
    return value


def nonnegative(value, label):
    if not isinstance(value, int) or value < 0:
        raise SmokeFailure(f"{label} must be a nonnegative integer")
    return value


def read_bounded(path, maximum, label):
    try:
        with Path(path).open("rb") as source:
            body = source.read(maximum + 1)
    except OSError as error:
        raise SmokeFailure(f"unable to read {label}: {error}") from error
    if len(body) > maximum:
        raise SmokeFailure(f"{label} exceeds {maximum} bytes")
    return body


def parse_event_stream(raw):
    if not raw:
        raise SmokeFailure("OpenCode produced no JSON events")
    events = []
    for line in raw.splitlines():
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise SmokeFailure("OpenCode stdout was not newline-delimited JSON") from error
        if not isinstance(event, dict):
            raise SmokeFailure("OpenCode event must be an object")
        event_type = exact_text(event.get("type"), "OpenCode event type")
        if event_type not in ("step_start", "text", "step_finish"):
            raise SmokeFailure(f"unsupported OpenCode event type: {event_type}")
        session_id = exact_text(event.get("sessionID"), "OpenCode event sessionID")
        part = event.get("part")
        if not isinstance(part, dict):
            raise SmokeFailure("OpenCode event lacks a part object")
        part_id = exact_text(part.get("id"), "OpenCode event part id")
        message_id = exact_text(part.get("messageID"), "OpenCode event messageID")
        if exact_text(part.get("sessionID"), "OpenCode event part sessionID") != session_id:
            raise SmokeFailure("OpenCode event part sessionID disagrees with its stream")
        expected_part_type = event_type.replace("_", "-")
        if exact_text(part.get("type"), "OpenCode event part type") != expected_part_type:
            raise SmokeFailure("OpenCode event type disagrees with its part type")
        events.append((event, session_id, message_id, part_id))
    if not events:
        raise SmokeFailure("OpenCode produced no JSON events")
    return events


def _assistant_message(session, session_id, message_id):
    info = session.get("info")
    messages = session.get("messages")
    if not isinstance(info, dict) or exact_text(info.get("id"), "session export id") != session_id:
        raise SmokeFailure("session export does not bind the event session")
    if not isinstance(messages, list):
        raise SmokeFailure("session export lacks messages")
    matches = []
    for message in messages:
        if not isinstance(message, dict) or not isinstance(message.get("info"), dict):
            continue
        candidate = message["info"]
        if candidate.get("role") == "assistant" and candidate.get("id") == message_id:
            matches.append(message)
    if len(matches) != 1:
        raise SmokeFailure("event stream does not identify exactly one exported assistant message")
    assistant = matches[0]
    if exact_text(assistant["info"].get("sessionID"), "assistant sessionID") != session_id:
        raise SmokeFailure("assistant message does not bind the event session")
    return info, assistant


def validate_archived_session(raw, session_body, expected_model):
    """Validate the observed OpenCode v1.18 stream/export relation.

    This intentionally admits only the event and export fields observed in the
    archived availability smoke. It does not recursively search arbitrary
    JSON for a model string, and it does not promise compatibility with a
    future OpenCode export schema without a reviewed extension.
    """
    provider, model = exact_model(expected_model).split("/", 1)
    events = parse_event_stream(raw)
    session_ids = {item[1] for item in events}
    message_ids = {item[2] for item in events}
    if len(session_ids) != 1 or len(message_ids) != 1:
        raise SmokeFailure("OpenCode stream spans more than one session or assistant message")
    session_id = next(iter(session_ids))
    message_id = next(iter(message_ids))
    try:
        session = json.loads(session_body)
    except json.JSONDecodeError as error:
        raise SmokeFailure("OpenCode session export was not JSON") from error
    if not isinstance(session, dict):
        raise SmokeFailure("OpenCode session export must be an object")
    session_info, assistant = _assistant_message(session, session_id, message_id)
    assistant_info = assistant["info"]
    session_model = session_info.get("model")
    if not isinstance(session_model, dict):
        raise SmokeFailure("session export lacks session model identity")
    if session_model.get("providerID") != provider or session_model.get("id") != model:
        raise SmokeFailure("session model identity does not match the requested model")
    if assistant_info.get("providerID") != provider or assistant_info.get("modelID") != model:
        raise SmokeFailure("assistant model identity does not match the requested model")
    parts = assistant.get("parts")
    if not isinstance(parts, list):
        raise SmokeFailure("assistant export lacks parts")
    exported_parts = {}
    for part in parts:
        if not isinstance(part, dict):
            raise SmokeFailure("assistant export part must be an object")
        part_id = exact_text(part.get("id"), "assistant export part id")
        if part_id in exported_parts:
            raise SmokeFailure("assistant export repeats a part id")
        exported_parts[part_id] = part
    seen_part_ids = set()
    saw_text = False
    finish_event = None
    for event, event_session_id, event_message_id, part_id in events:
        if event_session_id != session_id or event_message_id != message_id:
            raise SmokeFailure("OpenCode stream binding changed during validation")
        if part_id in seen_part_ids:
            raise SmokeFailure("OpenCode stream repeats a part id")
        seen_part_ids.add(part_id)
        if part_id not in exported_parts or canonical(event["part"]) != canonical(exported_parts[part_id]):
            raise SmokeFailure("OpenCode stream part does not exactly match the exported assistant part")
        if event["type"] == "text":
            if not isinstance(event["part"].get("text"), str) or not event["part"]["text"]:
                raise SmokeFailure("OpenCode text event lacks nonempty completion text")
            saw_text = True
        if event["type"] == "step_finish":
            if finish_event is not None:
                raise SmokeFailure("OpenCode stream has more than one finish event")
            finish_event = event
    if not saw_text or finish_event is None or assistant_info.get("finish") != "stop":
        raise SmokeFailure("OpenCode availability stream does not prove a stopped text completion")
    finish_part = finish_event["part"]
    if finish_part.get("reason") != "stop":
        raise SmokeFailure("OpenCode finish event was not a stop")
    tokens = assistant_info.get("tokens")
    if not isinstance(tokens, dict) or canonical(finish_part.get("tokens")) != canonical(tokens):
        raise SmokeFailure("OpenCode finish usage does not exactly match the exported assistant usage")
    usage = {
        "total": nonnegative(tokens.get("total"), "assistant total tokens"),
        "input": nonnegative(tokens.get("input"), "assistant input tokens"),
        "output": nonnegative(tokens.get("output"), "assistant output tokens"),
        "reasoning": nonnegative(tokens.get("reasoning"), "assistant reasoning tokens"),
    }
    cache = tokens.get("cache")
    if not isinstance(cache, dict):
        raise SmokeFailure("assistant usage lacks cache counters")
    usage["cache_read"] = nonnegative(cache.get("read"), "assistant cache-read tokens")
    usage["cache_write"] = nonnegative(cache.get("write"), "assistant cache-write tokens")
    if usage["total"] != usage["input"] + usage["output"] + usage["reasoning"]:
        raise SmokeFailure("assistant total tokens disagree with the observed usage components")
    if finish_part.get("cost") != assistant_info.get("cost"):
        raise SmokeFailure("OpenCode finish cost does not match the exported assistant cost")
    return {
        "schema": "semaprax.opencode-provider-smoke-validation.v1",
        "model": expected_model,
        "session_id": session_id,
        "assistant_message_id": message_id,
        "raw_events_sha256": sha256(raw),
        "session_export_sha256": sha256(session_body),
        "completion": "stopped_text_completion",
        "usage": usage,
        "cost": finish_part["cost"],
        "model_identity": "matching OpenCode session/export self-report; not cryptographic attestation",
    }


def validate_archive(raw_events_path, session_export_path, expected_model):
    raw = read_bounded(raw_events_path, MAX_OUTPUT_BYTES, "raw OpenCode event stream")
    session_body = read_bounded(session_export_path, MAX_SESSION_EXPORT_BYTES, "OpenCode session export")
    return validate_archived_session(raw, session_body, expected_model)


def terminate_process_group(process):
    """Terminate the launched process and its POSIX process group when present."""
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    elif process.poll() is None:
        process.kill()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        # A child which ignores the first signal must not survive this adapter.
        process.kill()
        process.wait()


def capture_stdout(command_line, output, timeout_seconds, max_output_bytes):
    """Run without a shell and cap the captured raw stdout stream in flight.

    The output is created before the child starts.  Therefore an unwritable or
    pre-existing evidence destination cannot leave a provider process alive.
    """
    try:
        captured = output.open("xb")
    except OSError as error:
        raise SmokeFailure(f"unable to create raw event output: {error}") from error
    process = None
    selector = None
    try:
        try:
            process = subprocess.Popen(
                command_line,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                start_new_session=(os.name == "posix"),
            )
        except OSError as error:
            raise SmokeFailure(f"unable to start OpenCode: {error}") from error
        assert process.stdout is not None
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        started = time.monotonic()
        total = 0
        with captured:
            while selector.get_map():
                remaining = timeout_seconds - (time.monotonic() - started)
                if remaining <= 0:
                    raise SmokeFailure("OpenCode availability smoke timed out")
                for key, _ in selector.select(min(remaining, 0.1)):
                    chunk = os.read(key.fd, min(65_536, max_output_bytes + 1 - total))
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    total += len(chunk)
                    if total > max_output_bytes:
                        raise SmokeFailure("OpenCode JSON event stream exceeded --max-output-bytes")
                    captured.write(chunk)
        return_code = process.wait(timeout=max(0.1, timeout_seconds - (time.monotonic() - started)))
        if return_code != 0:
            raise SmokeFailure(f"OpenCode exited with status {return_code}")
        return output.read_bytes()
    except SmokeFailure:
        if process is not None:
            terminate_process_group(process)
        raise
    except subprocess.TimeoutExpired as error:
        if process is not None:
            terminate_process_group(process)
        raise SmokeFailure("OpenCode availability smoke timed out") from error
    except OSError as error:
        if process is not None:
            terminate_process_group(process)
        raise SmokeFailure(f"unable to capture OpenCode JSON events: {error}") from error
    except BaseException:
        if process is not None:
            terminate_process_group(process)
        raise
    finally:
        if selector is not None:
            selector.close()
        if process is not None and process.stdout is not None:
            process.stdout.close()
        if not captured.closed:
            captured.close()


def plan(arguments):
    model = exact_model(arguments.model)
    prompt = load_frozen_prompt(arguments.prompt_file, arguments.prompt_sha256)
    sandbox = Path(arguments.sandbox).resolve(strict=True)
    line = command(arguments.opencode, model, sandbox, prompt)
    return {
        "schema": "semaprax.opencode-provider-smoke.v1",
        "mode": "opencode-managed-auth-store",
        "model": model,
        "prompt_sha256": arguments.prompt_sha256,
        "tool_policy": "agent-specific permission {*} deny",
        "command": line,
        "network_calls_made": 0,
        "provider_usage": "unknown_until_a_provider_event_is_archived",
    }


def execute(arguments):
    if not arguments.raw_events_output:
        raise SmokeFailure("--execute requires --raw-events-output")
    model = exact_model(arguments.model)
    timeout_seconds = positive(arguments.timeout_seconds, "--timeout-seconds", MAX_TIMEOUT_SECONDS)
    max_output_bytes = positive(arguments.max_output_bytes, "--max-output-bytes", MAX_OUTPUT_BYTES)
    prompt = load_frozen_prompt(arguments.prompt_file, arguments.prompt_sha256)
    sandbox, policy_path, policy_bytes = prepare_empty_sandbox(arguments.sandbox)
    output = output_path_outside_sandbox(arguments.raw_events_output, sandbox)
    line = command(arguments.opencode, model, sandbox, prompt)
    raw = capture_stdout(line, output, timeout_seconds, max_output_bytes)
    return {
        "schema": "semaprax.opencode-provider-smoke.v1",
        "mode": "opencode-managed-auth-store",
        "model": model,
        "prompt_sha256": arguments.prompt_sha256,
        "policy_path": str(policy_path),
        "policy_sha256": sha256(policy_bytes),
        "raw_events_path": str(output),
        "raw_events_sha256": sha256(raw),
        "raw_events_bytes": len(raw),
        "completion": "unverified: validate an explicit OpenCode session export before claiming completion",
        "model_identity": "unverified until a matching OpenCode session export is archived",
        "provider_usage": "unknown: this smoke does not infer billing from local bytes",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prompt-file", required=True)
    parser.add_argument("--prompt-sha256", required=True)
    parser.add_argument("--sandbox", required=True)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--opencode", default=DEFAULT_OPENCODE)
    parser.add_argument("--execute", action="store_true", help="perform the one provider call")
    parser.add_argument("--raw-events-output")
    parser.add_argument("--validate-raw-events", help="archived raw JSON event stream to validate without a provider call")
    parser.add_argument("--session-export", help="archived `opencode export` JSON for --validate-raw-events")
    parser.add_argument("--timeout-seconds", type=int, default=60)
    parser.add_argument("--max-output-bytes", type=int, default=262_144)
    arguments = parser.parse_args()
    try:
        if arguments.validate_raw_events or arguments.session_export:
            if arguments.execute:
                raise SmokeFailure("--execute cannot be combined with archived-session validation")
            if not arguments.validate_raw_events or not arguments.session_export:
                raise SmokeFailure("archived-session validation requires both --validate-raw-events and --session-export")
            result = validate_archive(arguments.validate_raw_events, arguments.session_export, arguments.model)
        else:
            result = execute(arguments) if arguments.execute else plan(arguments)
    except SmokeFailure as error:
        print(f"OpenCode provider smoke refused: {error}", file=sys.stderr)
        raise SystemExit(2)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
