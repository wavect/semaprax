"""Explicit, bounded Ollama loopback transport; never provisions or pulls models.

A daemon-reported digest is an identity under the operator's trust in their
local daemon, not independent model-origin proof or an egress sandbox. A live
experiment must additionally bind that trust/egress boundary in its review.
Mock-server tests exercise the wire implementation, never a real model trial.
"""
from __future__ import annotations

import hashlib
import http.client
import json
import math
import re
import socket
import time
import threading
from dataclasses import dataclass
from urllib.parse import urlsplit

from .transport import SolverTransport
from .budget import BudgetLedger
from .contracts import SolverRequest, SolverResponse, TranscriptEntry

MAX_HTTP_BYTES = 128 * 1024
MAX_PROMPT_BYTES = 30 * 1024
CONTEXT_TOKENS = 8192
MAX_TIMEOUT_SECONDS = 120
SHA256 = re.compile(r"(?:sha256:)?[0-9a-f]{64}\Z")


class LocalTransportError(ValueError):
    """Refusal or ambiguous attempt; the caller must not silently retry it."""

    def __init__(self, reason: str, *, dispatched: bool = False, observation=None):
        super().__init__(reason)
        self.dispatched = dispatched
        self.observation = observation


def canonical(value) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"),
                       ensure_ascii=True, allow_nan=False) + "\n").encode("ascii")


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def strict_json(data: bytes):
    def pairs(rows):
        result = {}
        for name, value in rows:
            if name in result:
                raise LocalTransportError("duplicate_json_key")
            result[name] = value
        return result
    def bad_constant(_):
        raise LocalTransportError("nonfinite_json_number")
    try:
        return json.loads(data.decode("utf-8"), object_pairs_hook=pairs,
                          parse_constant=bad_constant)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise LocalTransportError("invalid_json") from error


def endpoint(value: str) -> tuple[str, int]:
    try:
        parts = urlsplit(value)
        port = parts.port
    except (ValueError, TypeError) as error:
        raise LocalTransportError("invalid_loopback_endpoint") from error
    # No DNS, alternate integer/octal IP forms, credentials, proxy or redirects.
    if (parts.scheme != "http" or parts.hostname != "127.0.0.1" or
            parts.username is not None or parts.password is not None or
            parts.path or parts.query or parts.fragment or port is None or
            not 1 <= port <= 65535 or value != f"http://127.0.0.1:{port}"):
        raise LocalTransportError("literal_loopback_endpoint_required")
    return "127.0.0.1", port


def nonnegative(value, name):
    if type(value) is not int or value < 0:
        raise LocalTransportError("invalid_usage:" + name)
    return value


@dataclass(frozen=True)
class ModelPin:
    name: str
    sha256: str
    show_sha256: str
    daemon_version: str

    def __post_init__(self):
        if (not isinstance(self.name, str) or ":" not in self.name or
                self.name.rsplit(":", 1)[-1] in {"latest", "cloud"} or
                any(x in self.name for x in ("@", "://", "\\", "..")) or
                not re.fullmatch(r"[a-zA-Z0-9_./:-]{1,160}", self.name)):
            raise LocalTransportError("explicit_local_model_tag_required")
        if not isinstance(self.sha256, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", self.sha256):
            raise LocalTransportError("full_model_digest_required")
        if not isinstance(self.show_sha256, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", self.show_sha256):
            raise LocalTransportError("full_model_metadata_digest_required")
        if not isinstance(self.daemon_version, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:[-+][a-zA-Z0-9.-]+)?", self.daemon_version):
            raise LocalTransportError("explicit_daemon_version_required")

    def to_dict(self):
        return dict(name=self.name, sha256=self.sha256,
                    show_sha256=self.show_sha256, daemon_version=self.daemon_version)


class Client:
    def __init__(self, address: str, timeout_seconds: float = 120):
        self.host, self.port = endpoint(address)
        if (type(timeout_seconds) not in (int, float) or
                not math.isfinite(timeout_seconds) or
                not 0 < timeout_seconds <= MAX_TIMEOUT_SECONDS):
            raise LocalTransportError("invalid_http_timeout")
        self.address = address
        self.timeout = timeout_seconds
        self.last_generation_started = False

    def request(self, path: str, body=None):
        allowed = {("/api/version", False), ("/api/tags", False),
                   ("/api/show", True), ("/api/generate", True)}
        if (path, body is not None) not in allowed:
            raise LocalTransportError("endpoint_not_admitted")
        raw_request = canonical(body) if body is not None else None
        if raw_request is not None and len(raw_request) > MAX_HTTP_BYTES:
            raise LocalTransportError("request_byte_bound")
        deadline = time.monotonic() + self.timeout
        connection = http.client.HTTPConnection(self.host, self.port, timeout=self.timeout)
        dispatched = False
        timer = None
        try:
            # http.client does not consult HTTP(S)_PROXY, .netrc, or credentials.
            connection.connect()
            sock = connection.sock
            def expire():
                try:
                    sock.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
            timer = threading.Timer(max(0, deadline - time.monotonic()), expire)
            timer.daemon = True
            timer.start()
            if path == "/api/generate":
                # Once sending starts, an error is potentially billed/executed;
                # even a broken pipe cannot be safely reclassified as no call.
                dispatched = True
                self.last_generation_started = True
            connection.request("POST" if body is not None else "GET", path,
                               body=raw_request, headers={"Content-Type": "application/json",
                               "Accept": "application/json", "Connection": "close"})
            sock = connection.sock
            response = connection.getresponse()
            if response.status != 200:
                raise LocalTransportError(f"http_status:{response.status}", dispatched=dispatched)
            if response.getheader("Content-Encoding", "identity") != "identity":
                raise LocalTransportError("encoded_response_refused", dispatched=dispatched)
            length = response.getheader("Content-Length")
            if length is not None and (not length.isdigit() or int(length) > MAX_HTTP_BYTES):
                raise LocalTransportError("response_byte_bound", dispatched=dispatched)
            data = bytearray()
            while True:
                if response.isclosed():
                    break
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise LocalTransportError("http_deadline", dispatched=dispatched)
                # The response owns the socket after Connection: close. The
                # socket itself is retained so a trickling peer cannot reset
                # the overall wall deadline on each read.
                sock.settimeout(remaining)
                chunk = response.read1(min(8192, MAX_HTTP_BYTES + 1 - len(data)))
                if not chunk:
                    break
                data.extend(chunk)
                if len(data) > MAX_HTTP_BYTES:
                    raise LocalTransportError("response_byte_bound", dispatched=dispatched)
            if length is not None and len(data) != int(length):
                raise LocalTransportError("truncated_http_body", dispatched=dispatched)
            parsed = strict_json(bytes(data))
            if not isinstance(parsed, dict) or "error" in parsed:
                raise LocalTransportError("invalid_daemon_response", dispatched=dispatched)
            if len(canonical(parsed)) > MAX_HTTP_BYTES:
                raise LocalTransportError("decoded_response_byte_bound", dispatched=dispatched)
            return parsed
        except LocalTransportError as error:
            error.dispatched = error.dispatched or dispatched
            raise
        except (OSError, http.client.HTTPException, ValueError) as error:
            raise LocalTransportError(type(error).__name__, dispatched=dispatched) from error
        finally:
            if timer is not None:
                timer.cancel()
            connection.close()

    def inspect(self, name: str) -> ModelPin:
        # Validate the name even before contacting the daemon.
        ModelPin(name, "sha256:" + "0" * 64, "sha256:" + "0" * 64, "0.0.0")
        version = self.request("/api/version").get("version")
        models = self.request("/api/tags").get("models")
        if not isinstance(models, list):
            raise LocalTransportError("invalid_model_inventory")
        matches = [m for m in models if isinstance(m, dict) and m.get("name") == name]
        if len(matches) != 1:
            raise LocalTransportError("model_not_uniquely_installed")
        model = matches[0]
        value = model.get("digest")
        if not isinstance(value, str) or not SHA256.fullmatch(value):
            raise LocalTransportError("invalid_model_digest")
        show = self.request("/api/show", {"model": name, "verbose": False})
        if any(m.get(key) for m in (model, show) for key in ("remote_host", "remote_model")):
            raise LocalTransportError("cloud_model_refused")
        if (type(model.get("size")) is not int or model["size"] <= 0 or
                not isinstance(show.get("details"), dict) or
                show["details"].get("format") != "gguf" or
                show["details"].get("family") != "qwen2" or
                not isinstance(show.get("capabilities"), list) or
                "completion" not in show["capabilities"]):
            raise LocalTransportError("local_qwen2_gguf_completion_required")
        # /api/show has no sampling output: binds template, parameters,
        # architecture and all other returned model metadata, not just a tag.
        self.last_inspection = {"model_inventory_row": model, "model_show": show, "version": version}
        return ModelPin(name, "sha256:" + value.removeprefix("sha256:"),
                        digest(canonical(show)), version)

    def verify(self, pin: ModelPin):
        if self.inspect(pin.name) != pin:
            raise LocalTransportError("model_or_daemon_identity_drift")


def raw_prompt(prompt: str) -> str:
    return ("<|im_start|>system\nYou are a coding assistant. Return only the requested JSON."
            "<|im_end|>\n<|im_start|>user\n" + prompt +
            "<|im_end|>\n<|im_start|>assistant\n")


class LocalOllamaTransport(SolverTransport):
    """One real HTTP generation attempt. Invalid JSON is an outcome, not repair.

    Retries are deliberately not consumed automatically. A connection loss,
    timeout or malformed usage stops the study with unknown usage preserved.
    All control arms have the same policy. Only constrained uses format=schema.
    """
    def __init__(self, client: Client, pin: ModelPin, paths: tuple[str, ...], constrained: bool):
        self.client, self.pin = client, pin
        if not paths or len(paths) != len(set(paths)):
            raise LocalTransportError("invalid_candidate_inventory")
        for path in paths:
            if not re.fullmatch(r"src/[a-z][a-z0-9_]*\.spx", path):
                raise LocalTransportError("invalid_candidate_path")
        if type(constrained) is not bool:
            raise LocalTransportError("boolean_control_required")
        self.paths, self.constrained = paths, constrained
        self.observation = None

    @property
    def response_schema(self):
        return {"type": "object", "properties": {"files": {
            "type": "object", "properties": {p: {"type": "string"} for p in self.paths},
            "required": list(self.paths), "additionalProperties": False}},
            "required": ["files"], "additionalProperties": False}

    def complete(self, request: SolverRequest) -> SolverResponse:
        if request.model.to_dict() != {"provider": "ollama-local", "model": self.pin.name,
                                       "revision": self.pin.sha256}:
            raise LocalTransportError("request_model_pin_mismatch")
        self.observation = None
        self.client.last_generation_started = False
        for field in ("max_prompt_tokens", "max_completion_tokens", "max_total_tokens", "max_retries"):
            nonnegative(getattr(request.budget, field), field)
        if not isinstance(request.prompt, str):
            raise LocalTransportError("text_prompt_required")
        if (request.budget.max_cost_usd != 0 or request.pricing.input_usd_per_1k != 0 or
                request.pricing.output_usd_per_1k != 0):
            raise LocalTransportError("zero_provider_spend_only")
        sampling = request.sampling
        if (type(sampling.seed) is not int or not 0 <= sampling.seed < 2**31 or
                type(sampling.max_output_tokens) is not int or sampling.max_output_tokens <= 0 or
                type(sampling.temperature) not in (int, float) or
                not math.isfinite(sampling.temperature) or not 0 <= sampling.temperature <= 2 or
                type(sampling.top_p) not in (int, float) or
                not math.isfinite(sampling.top_p) or not 0 < sampling.top_p <= 1):
            raise LocalTransportError("invalid_sampling")
        prompt = raw_prompt(request.prompt)
        prompt_bytes = len(prompt.encode("utf-8"))
        # This path is restricted to Qwen2 byte-BPE with raw ChatML. The
        # conservative byte ceiling includes template tokens and 32 spare
        # special tokens. No server-side template or automatic truncation.
        upper_bound = prompt_bytes + 32
        if (prompt_bytes > MAX_PROMPT_BYTES or upper_bound > request.budget.max_prompt_tokens or
                sampling.max_output_tokens > request.budget.max_completion_tokens or
                upper_bound + sampling.max_output_tokens > request.budget.max_total_tokens or
                upper_bound + sampling.max_output_tokens > CONTEXT_TOKENS):
            raise LocalTransportError("pre_dispatch_token_bound")
        self.client.verify(self.pin)
        body = {"model": self.pin.name, "prompt": prompt, "raw": True, "stream": False,
                "keep_alive": 0, "options": {"temperature": sampling.temperature,
                "top_p": sampling.top_p, "seed": sampling.seed,
                "num_predict": sampling.max_output_tokens, "num_ctx": CONTEXT_TOKENS}}
        if self.constrained:
            body["format"] = self.response_schema
        start = time.monotonic()
        response = self.client.request("/api/generate", body)
        self.observation = {"request": body, "response": response,
                            "wall_ms": (time.monotonic() - start) * 1000,
                            "usage_status": "unverified"}
        try:
            if (response.get("model") != self.pin.name or response.get("done") is not True or
                    response.get("done_reason") not in {"stop", "length"} or
                    not isinstance(response.get("response"), str)):
                raise LocalTransportError("incomplete_or_unbound_generation")
            prompt_tokens = nonnegative(response.get("prompt_eval_count"), "prompt_eval_count")
            output_tokens = nonnegative(response.get("eval_count"), "eval_count")
            for name in ("total_duration", "load_duration", "prompt_eval_duration", "eval_duration"):
                nonnegative(response.get(name), name)
            if prompt_tokens == 0 or prompt_tokens > upper_bound or output_tokens > sampling.max_output_tokens:
                raise LocalTransportError("observed_usage_exceeds_reservation")
            self.client.verify(self.pin)
            ledger = BudgetLedger(request.budget)
            ledger.charge_attempt(prompt_tokens, output_tokens, 0.0, False)
        except Exception as error:
            raise LocalTransportError(str(error), dispatched=True, observation=self.observation) from error
        self.observation["usage_status"] = "verified"
        text = response["response"]
        try:
            answer = strict_json(text.encode("utf-8"))
            valid = (isinstance(answer, dict) and set(answer) == {"files"} and
                     isinstance(answer["files"], dict) and set(answer["files"]) == set(self.paths) and
                     all(isinstance(v, str) for v in answer["files"].values()))
        except LocalTransportError:
            valid = False
        # No stripping fences, inferred paths, reference fallback or second
        # model call. {} fails the existing orchestrator's exact-path gate.
        files = answer["files"] if valid and response["done_reason"] == "stop" else {}
        self.observation["candidate_valid"] = bool(files)
        return SolverResponse(files, ledger.usage, [
            TranscriptEntry(1, "prompt", "final", prompt),
            TranscriptEntry(1, "response", "final", text, prompt_tokens, output_tokens, 0.0)])
