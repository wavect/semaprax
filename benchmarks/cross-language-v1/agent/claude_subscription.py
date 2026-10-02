"""Explicit native Claude subscription transport for live-pilot v1 only."""
from __future__ import annotations
import base64
import os
import pathlib
import platform
import re
import selectors
import signal
import subprocess
import tempfile
import time

from . import pilot_protocol as p

MAX_WIRE = 1024 * 1024
MANAGED = (
    "/Library/Application Support/ClaudeCode/managed-settings.json",
    "/Library/Application Support/ClaudeCode/managed-settings.d",
    "/Library/Application Support/ClaudeCode/managed-mcp.json",
    "/Library/Managed Preferences/com.anthropic.claudecode.plist",
    "/etc/claude-code/managed-settings.json", "/etc/claude-code/managed-settings.d",
    "/etc/claude-code/managed-mcp.json",
)


class Failure(ValueError):
    def __init__(self, reason, receipt):
        super().__init__(reason)
        self.receipt = receipt


def capture(argv, cwd, env, prompt, seconds, maximum=MAX_WIRE, on_started=lambda: None):
    """Drain both pipes concurrently; deadline includes descendants holding pipes."""
    start = time.monotonic()
    out = {"stdout": bytearray(), "stderr": bytearray()}
    reason = None
    # A private regular stdin avoids pipe write-vs-read deadlock entirely.
    with tempfile.TemporaryFile(dir=cwd) as request, selectors.DefaultSelector() as selector:
        request.write(prompt)
        request.seek(0)
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=request, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True, close_fds=True)
        try:
            on_started()
            for stream, name in ((process.stdout, "stdout"), (process.stderr, "stderr")):
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ, name)
            while selector.get_map() or process.poll() is None:
                remaining = seconds - (time.monotonic() - start)
                if remaining <= 0:
                    reason = "provider_deadline"
                    break
                for key, _ in selector.select(min(remaining, 0.05)):
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    room = maximum - sum(map(len, out.values()))
                    out[key.data].extend(chunk[:room])
                    if len(chunk) > room:
                        reason = "provider_output_bound"
                        break
                if reason:
                    break
        finally:
            # Also remove descendants after a successful parent exit. Nothing
            # from this one-use transport is allowed to outlive its invocation.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=5)
            process.stdout.close()
            process.stderr.close()
    return {"exit_code": process.returncode, "failure": reason,
            "duration_ms": round((time.monotonic() - start) * 1000),
            **{name + "_base64": base64.b64encode(data).decode() for name, data in out.items()}}


def decode(wire, model, limits):
    envelope = p.strict_json(wire)
    if not isinstance(envelope, dict):
        raise ValueError("claude_envelope_shape")
    if (envelope.get("type") != "result" or envelope.get("subtype") != "success"
            or envelope.get("is_error") is not False or type(envelope.get("num_turns")) is not int
            or envelope["num_turns"] != 1 or envelope.get("permission_denials") != []
            or envelope.get("stop_reason") != "end_turn"
            or envelope.get("terminal_reason") != "completed"
            or envelope.get("queued_turn_count") != 0):
        raise ValueError("claude_single_turn_refused")
    model_usage = envelope.get("modelUsage")
    if not isinstance(model_usage, dict) or set(model_usage) != {model["reported_model"]}:
        raise ValueError("claude_reported_model_mismatch")
    observed = model_usage[model["reported_model"]]
    if not isinstance(observed, dict) or observed.get("provider") != "firstParty" or observed.get("canonicalModel") != model["reported_model"]:
        raise ValueError("claude_model_provenance_refused")
    agents = envelope.get("subagent_stats")
    if not isinstance(agents, dict) or agents.get("spawned") != 0:
        raise ValueError("claude_subagent_refused")
    usage = envelope.get("usage")
    if not isinstance(usage, dict):
        raise ValueError("claude_usage_missing")
    tokens = {}
    for key in ("input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"):
        value = usage.get(key)
        if type(value) is not int or value < 0:
            raise ValueError("claude_usage_missing")
        tokens[key] = value
    if sum(tokens.values()) > limits["max_reported_tokens"]:
        raise ValueError("reported_token_budget_exceeded")
    cost = envelope.get("total_cost_usd")
    if type(cost) not in (float, int) or not 0 <= cost <= limits["max_estimated_usd"]:
        raise ValueError("reported_estimated_cost_budget_exceeded")
    result = envelope.get("result")
    if not isinstance(result, str) or len(result.encode()) > limits["max_result_bytes"]:
        raise ValueError("claude_result_bound")
    # Two documented transport frames, neither changes candidate bytes.
    framed = result
    if framed.startswith("```json\n") and framed.endswith("\n```"):
        framed = framed[8:-4]
    candidate = p.strict_json(framed)
    p.exact(candidate, (p.CANDIDATE,), "candidate_file_set_refused")
    if not isinstance(candidate[p.CANDIDATE], str) or not candidate[p.CANDIDATE] or "\x00" in candidate[p.CANDIDATE]:
        raise ValueError("candidate_text_refused")
    return {"candidate_files": candidate, "usage": tokens, "estimated_api_cost_usd": cost,
            "subscription_invoice_cost_usd": None, "reported_model": model_usage,
            "requested_model": model["requested_model"], "requested_snapshot_observed_directly": model["requested_model"] == model["reported_model"]}


def native_identity():
    machine = platform.machine().lower()
    system = platform.system().lower()
    native = "darwin-arm64" if system == "darwin" and machine == "arm64" else (
        "linux-arm64" if system == "linux" and machine in ("arm64", "aarch64") else "unsupported")
    boot_id = None
    if native == "linux-arm64":
        boot_id = p.provenance.read_regular(pathlib.Path("/proc/sys/kernel/random/boot_id"), 64).decode().strip()
    return {"native_platform": native, "kernel_release": platform.release(), "boot_id": boot_id}


def model_arguments(plan, model):
    return ["--print", "--output-format", "json", "--tools", "",
            "--no-session-persistence", "--safe-mode", "--restricted", "--strict-mcp-config",
            "--permission-prompts", "none", "--prompt-suggestions", "false",
            "--max-budget-usd", str(plan["configuration"]["limits"]["max_estimated_usd"]),
            "--model", model["requested_model"], "--system-prompt", plan["system_prompt"]]


class ClaudeSubscription:
    """Per-host home/login grants; a host label can never redirect native dispatch."""
    def __init__(self, host, scratch):
        self.host = dict(host)
        self.observation = native_identity()
        if any(self.observation[key] != host[key] for key in self.observation):
            raise ValueError("provider_native_host_mismatch")
        self.executable = pathlib.Path(host["executable"])
        self.home = pathlib.Path(host["home"])
        self.scratch = pathlib.Path(scratch)
        if any(not x.is_absolute() or x != x.resolve() for x in (self.executable, self.home, self.scratch)):
            raise ValueError("explicit_canonical_paths_required")
        if not self.home.is_dir() or not self.scratch.is_dir() or not re.fullmatch(r"[A-Za-z0-9_.-]{1,256}", host["login"]):
            raise ValueError("subscription_identity_refused")
        self.login = host["login"]
        self.used = False

    def complete(self, plan, model, on_started=lambda: None):
        if self.used:
            raise ValueError("transport_already_consumed")
        self.used = True
        for path in MANAGED:
            try:
                os.lstat(path)
            except FileNotFoundError:
                continue
            raise ValueError("managed_claude_settings_refused")
        binary = p.provenance.read_regular(self.executable, 256 * 1024 * 1024)
        if p.digest(binary) != self.host["claude_sha256"]:
            raise ValueError("claude_executable_digest_mismatch")
        limits = plan["configuration"]["limits"]
        prompt = plan["prompt"].encode()
        if len(prompt) + len(plan["system_prompt"].encode()) > limits["max_request_bytes"]:
            raise ValueError("request_exceeds_frozen_budget")
        with tempfile.TemporaryDirectory(prefix="pilot-claude-", dir=self.scratch) as temporary:
            root = pathlib.Path(temporary)
            staged = root / "claude"
            staged.write_bytes(binary)
            staged.chmod(0o500)
            del binary
            env = {"HOME": str(self.home), "USER": self.login, "LOGNAME": self.login,
                   "PATH": "/usr/bin:/bin", "TMPDIR": str(root), "DISABLE_AUTOUPDATER": "1",
                   "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1", "CLAUDE_CODE_SAFE_MODE": "1"}
            version = capture([str(staged), "--version"], root, env, b"", 5)
            expected = plan["configuration"]["cli_version"] + " (Claude Code)"
            if (version["failure"] or version["exit_code"] != 0
                    or base64.b64decode(version["stdout_base64"]).decode().strip() != expected):
                raise Failure("claude_version_mismatch", dict(version, dispatches=0))
            argv = [str(staged), *model_arguments(plan, model)]
            receipt = capture(argv, root, env, prompt, limits["deadline_seconds"], on_started=on_started)
            receipt.update(transport="claude-subscription-print-json.pilot.v1", dispatches=1,
                           cli_sha256=self.host["claude_sha256"], prompt_sha256=p.digest(prompt),
                           requested_model=model["requested_model"], argv=argv[1:],
                           provider_host=self.observation, host_authority_sha256=p.digest(p.canonical(self.host)),
                           cli_version=plan["configuration"]["cli_version"], cli_version_receipt=version,
                           token_cap_kind="post_response_admission_not_provider_hard_limit",
                           internal_provider_retries="not_observable")
            try:
                if receipt["failure"] or receipt["exit_code"] != 0:
                    raise ValueError(receipt["failure"] or "provider_process_failed")
                response = decode(base64.b64decode(receipt["stdout_base64"]), model, limits)
            except (ValueError, TypeError, KeyError) as error:
                raise Failure(str(error), receipt) from error
            response["transport_receipt"] = receipt
            return response
