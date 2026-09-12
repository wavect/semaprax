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


class SmokeFailure(Exception):
    pass


def sha256(body):
    return hashlib.sha256(body).hexdigest()


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


def _event_names_model(value, expected):
    if isinstance(value, list):
        return any(_event_names_model(item, expected) for item in value)
    if not isinstance(value, dict):
        return False
    for key in ("model", "model_id", "modelID"):
        if value.get(key) == expected:
            return True
    provider = value.get("provider") or value.get("provider_id") or value.get("providerID")
    model = value.get("model_id") or value.get("modelID") or value.get("id")
    if isinstance(provider, str) and isinstance(model, str) and f"{provider}/{model}" == expected:
        return True
    return any(_event_names_model(item, expected) for item in value.values())


def verify_raw_events(raw, expected_model):
    """Require newline-delimited JSON and a matching model self-report.

    OpenCode documents JSON event output but not a stable event schema.  This
    deliberately accepts only documented-style model identity fields and
    fails closed on a future incompatible event shape while retaining the raw
    bytes for operator inspection.
    """
    if not raw:
        raise SmokeFailure("OpenCode produced no JSON events")
    found_identity = False
    for line in raw.splitlines():
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise SmokeFailure("OpenCode stdout was not newline-delimited JSON") from error
        found_identity = found_identity or _event_names_model(event, expected_model)
    if not found_identity:
        raise SmokeFailure("OpenCode events did not identify the exact requested model")


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
    verify_raw_events(raw, model)
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
        "completion": "unverified: this availability smoke does not infer a completion from event fields",
        "model_identity": "matching OpenCode JSON self-report; not cryptographic attestation",
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
    parser.add_argument("--timeout-seconds", type=int, default=60)
    parser.add_argument("--max-output-bytes", type=int, default=262_144)
    arguments = parser.parse_args()
    try:
        result = execute(arguments) if arguments.execute else plan(arguments)
    except SmokeFailure as error:
        print(f"OpenCode provider smoke refused: {error}", file=sys.stderr)
        raise SystemExit(2)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
