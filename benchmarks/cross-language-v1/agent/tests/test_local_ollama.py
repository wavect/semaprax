"""Synthetic loopback HTTP controls. None of these tests runs a model."""
from __future__ import annotations
import copy
import dataclasses
import http.server
import json
import os
import pathlib
import socket
import sys
import threading
import time
import unittest
from unittest.mock import patch

SUITE = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(SUITE)) if str(SUITE) not in sys.path else None
from agent import local_ollama as transport
from agent.contracts import Budget, ModelIdentity, SamplingParams, PricingRates, SolverRequest

NAME = "qwen2.5-coder:7b"


class FakeDaemon:
    """Wire fixture only: returned source and usage are deliberately synthetic."""
    def __init__(self):
        self.calls = []
        self.version = "0.11.0"
        self.model = {"name": NAME, "model": NAME, "digest": "a" * 64, "size": 100}
        self.show = {"details": {"format": "gguf", "family": "qwen2"},
                     "capabilities": ["completion"], "template": "fixture template",
                     "parameters": "fixture parameters"}
        self.reply = {"model": NAME, "response": json.dumps({"files": {"src/candidate.spx": "fixture source"}}),
                      "done": True, "done_reason": "stop", "prompt_eval_count": 10, "eval_count": 12,
                      "total_duration": 100, "load_duration": 5, "prompt_eval_duration": 10, "eval_duration": 85}
        self.override = None
        self.after_generate = None

    def __enter__(self):
        owner = self
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_GET(self):
                self.respond()
            def do_POST(self):
                self.respond()
            def respond(self):
                body = None
                if self.command == "POST":
                    body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                owner.calls.append((self.path, body))
                if owner.override and owner.override(self):
                    return
                value = {"/api/version": {"version": owner.version},
                         "/api/tags": {"models": [owner.model]}, "/api/show": owner.show,
                         "/api/generate": owner.reply}[self.path]
                raw = transport.canonical(value)
                self.send_response(200)
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                try:
                    self.wfile.write(raw)
                except (BrokenPipeError, ConnectionResetError):
                    pass
                if self.path == "/api/generate" and owner.after_generate:
                    owner.after_generate()
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=lambda: self.server.serve_forever(poll_interval=0.01), daemon=True)
        self.thread.start()
        self.endpoint = f"http://127.0.0.1:{self.server.server_port}"
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)

    @property
    def generations(self):
        return [body for path, body in self.calls if path == "/api/generate"]


def request(pin, **kwargs):
    result = SolverRequest("fixture-task", "semaprax-project", "Implement the public fixture interface.",
        ModelIdentity("ollama-local", pin.name, pin.sha256), SamplingParams(0.2, 0.95, 1, 2048),
        Budget(32000, 16000, 48000, 2, 0), PricingRates(0, 0))
    return dataclasses.replace(result, **kwargs)


class LocalOllamaTests(unittest.TestCase):
    def make(self, daemon, constrained=False, timeout=120):
        client = transport.Client(daemon.endpoint, timeout)
        pin = client.inspect(NAME)
        return transport.LocalOllamaTransport(client, pin, ("src/candidate.spx",), constrained), pin

    def test_only_literal_loopback_explicit_port_is_allowed(self):
        for address in ("http://localhost:11434", "http://127.1:11434", "https://127.0.0.1:11434",
                        "http://127.0.0.1", "http://127.0.0.1:11434/", "http://127.0.0.1:11434?x=1",
                        "http://user@127.0.0.1:11434", "http://2130706433:11434", "http://[::1]:11434",
                        "http://127.0.0.1:00080", "http://127.0.0.1:65536"):
            with self.subTest(address=address), self.assertRaises(transport.LocalTransportError):
                transport.Client(address)
        self.assertEqual(transport.endpoint("http://127.0.0.1:11434"), ("127.0.0.1", 11434))

    def test_bounds_reject_invalid_deadlines_and_pin_types(self):
        for timeout in (True, 0, -1, 121, float("nan"), float("inf")):
            with self.subTest(timeout=timeout), self.assertRaises(transport.LocalTransportError):
                transport.Client("http://127.0.0.1:11434", timeout)
        for name in ("qwen", "qwen:latest", "qwen:cloud", "../qwen:7b", "https://host/model:7b"):
            with self.subTest(name=name), self.assertRaises(transport.LocalTransportError):
                transport.ModelPin(name, "sha256:" + "a" * 64, "sha256:" + "b" * 64, "0.11.0")
        with self.assertRaises(transport.LocalTransportError):
            transport.ModelPin(NAME, None, "sha256:" + "b" * 64, "0.11.0")

    def test_metadata_only_inspection_binds_complete_digest_and_show(self):
        with FakeDaemon() as daemon:
            client = transport.Client(daemon.endpoint)
            pin = client.inspect(NAME)
            self.assertEqual(pin.sha256, "sha256:" + "a" * 64)
            self.assertEqual(pin.show_sha256, transport.digest(transport.canonical(daemon.show)))
            self.assertEqual(len(daemon.generations), 0)
            self.assertEqual([x[0] for x in daemon.calls], ["/api/version", "/api/tags", "/api/show"])

    def test_non_generation_endpoints_cannot_pull_install_delete_or_chat(self):
        with FakeDaemon() as daemon:
            client = transport.Client(daemon.endpoint)
            for path in ("/api/pull", "/api/create", "/api/delete", "/api/chat", "http://elsewhere/api/generate"):
                with self.subTest(path=path), self.assertRaises(transport.LocalTransportError):
                    client.request(path, {})
            self.assertEqual(daemon.calls, [])

    def test_cloud_missing_inventory_and_wrong_architecture_refuse(self):
        for kind in ("cloud", "inventory", "architecture", "details", "capabilities", "version", "size", "digest"):
            with self.subTest(kind=kind), FakeDaemon() as daemon:
                if kind == "cloud": daemon.show["remote_host"] = "https://example.invalid"
                if kind == "inventory": daemon.model["name"] = "different:7b"
                if kind == "architecture": daemon.show["details"]["family"] = "other"
                if kind == "details": daemon.show["details"] = None
                if kind == "capabilities": daemon.show["capabilities"] = None
                if kind == "version": daemon.version = None
                if kind == "size": daemon.model["size"] = True
                if kind == "digest": daemon.model["digest"] = "abc"
                with self.assertRaises(transport.LocalTransportError):
                    transport.Client(daemon.endpoint).inspect(NAME)
                self.assertFalse(daemon.generations)

    def test_real_http_wire_fixture_returns_measured_usage_and_exact_candidate(self):
        with FakeDaemon() as daemon:
            runner, pin = self.make(daemon)
            result = runner.complete(request(pin))
            self.assertEqual(result.candidate_files, {"src/candidate.spx": "fixture source"})
            self.assertEqual((result.usage.prompt_tokens, result.usage.completion_tokens), (10, 12))
            self.assertEqual(result.usage.cost_usd, 0)
            self.assertEqual(len(daemon.generations), 1)
            self.assertNotIn("format", daemon.generations[0])
            self.assertTrue(daemon.generations[0]["raw"])
            self.assertFalse(daemon.generations[0]["stream"])
            self.assertEqual(daemon.generations[0]["options"], dict(temperature=0.2, top_p=0.95, seed=1, num_predict=2048, num_ctx=8192))
            self.assertEqual(runner.observation["usage_status"], "verified")

    def test_constrained_arm_alone_passes_exact_closed_files_schema(self):
        with FakeDaemon() as daemon:
            runner, pin = self.make(daemon, constrained=True)
            runner.complete(request(pin))
            schema = daemon.generations[0]["format"]
            self.assertFalse(schema["additionalProperties"])
            self.assertEqual(schema["properties"]["files"]["required"], ["src/candidate.spx"])
            self.assertFalse(schema["properties"]["files"]["additionalProperties"])

    def test_proxy_and_ambient_credential_variables_are_ignored(self):
        with FakeDaemon() as daemon, patch.dict(os.environ, {
            "HTTP_PROXY": "http://invalid.invalid:1", "HTTPS_PROXY": "http://invalid.invalid:1",
            "ALL_PROXY": "http://invalid.invalid:1", "OLLAMA_API_KEY": "synthetic-test-do-not-use"}):
            runner, pin = self.make(daemon)
            runner.complete(request(pin))
            self.assertEqual(len(daemon.generations), 1)
            self.assertNotIn("synthetic-test-do-not-use", repr(daemon.calls))

    def test_invalid_candidate_is_retained_failure_not_repair_or_retry(self):
        values = ("```json\n{}\n```", "not-json", '{"files":{"../secret":"x"}}',
                  '{"files":{"src/candidate.spx":10}}', '{"files":{},"extra":true}',
                  '{"files":{"src/candidate.spx":"x"},"files":{}}')
        for value in values:
            with self.subTest(value=value), FakeDaemon() as daemon:
                daemon.reply["response"] = value
                runner, pin = self.make(daemon)
                result = runner.complete(request(pin))
                self.assertEqual(result.candidate_files, {})
                self.assertEqual(result.usage.completion_tokens, 12)
                self.assertEqual(len(daemon.generations), 1)
                self.assertEqual(runner.observation["response"]["response"], value)

    def test_length_stop_is_failed_even_when_json_looks_complete(self):
        with FakeDaemon() as daemon:
            daemon.reply["done_reason"] = "length"
            runner, pin = self.make(daemon)
            self.assertEqual(runner.complete(request(pin)).candidate_files, {})

    def test_zero_spend_prompt_and_output_reservations_precede_generation(self):
        with FakeDaemon() as daemon:
            runner, pin = self.make(daemon)
            invalid = [request(pin, prompt="x" * 32768),
                       request(pin, pricing=PricingRates(0.1, 0)),
                       request(pin, budget=Budget(32000, 16000, 48000, 2, 1)),
                       request(pin, budget=Budget(True, 16000, 48000, 2, 0)),
                       request(pin, sampling=SamplingParams(0.2, 0.95, 1, 16001)),
                       request(pin, sampling=SamplingParams(float("nan"), 0.95, 1, 2048))]
            for value in invalid:
                with self.subTest(value=value.sampling), self.assertRaises(transport.LocalTransportError):
                    runner.complete(value)
            self.assertFalse(daemon.generations)

    def test_pre_dispatch_pin_drift_does_not_invoke_generation(self):
        for field in ("digest", "template", "daemon"):
            with self.subTest(field=field), FakeDaemon() as daemon:
                runner, pin = self.make(daemon)
                if field == "digest": daemon.model["digest"] = "b" * 64
                if field == "template": daemon.show["template"] += " changed"
                if field == "daemon": daemon.version = "0.11.1"
                with self.assertRaisesRegex(transport.LocalTransportError, "identity_drift"):
                    runner.complete(request(pin))
                self.assertFalse(daemon.generations)

    def test_post_dispatch_pin_drift_retains_ambiguous_observation(self):
        with FakeDaemon() as daemon:
            runner, pin = self.make(daemon)
            daemon.after_generate = lambda: daemon.model.update(digest="b" * 64)
            with self.assertRaises(transport.LocalTransportError) as caught:
                runner.complete(request(pin))
            self.assertTrue(caught.exception.dispatched)
            self.assertIsNotNone(caught.exception.observation)
            self.assertEqual(len(daemon.generations), 1)

    def test_malformed_usage_and_wrong_model_are_never_accepted(self):
        for field, value in (("eval_count", True), ("eval_count", -1), ("eval_count", 2049),
                             ("prompt_eval_count", 99999), ("prompt_eval_count", 0),
                             ("total_duration", None), ("done", False), ("model", "wrong:7b")):
            with self.subTest(field=field, value=value), FakeDaemon() as daemon:
                daemon.reply[field] = value
                runner, pin = self.make(daemon)
                with self.assertRaises(transport.LocalTransportError) as caught:
                    runner.complete(request(pin))
                self.assertTrue(caught.exception.dispatched)
                self.assertEqual(len(daemon.generations), 1)

    def test_http_redirect_error_and_oversize_refuse_without_followup(self):
        for status in (302, 401, 500, 200):
            with self.subTest(status=status), FakeDaemon() as daemon:
                def override(handler):
                    if handler.path != "/api/generate": return False
                    handler.send_response(status)
                    handler.send_header("Location", "https://example.invalid/")
                    handler.send_header("Content-Length", str(transport.MAX_HTTP_BYTES + 1))
                    handler.end_headers()
                    return True
                runner, pin = self.make(daemon)
                daemon.override = override
                with self.assertRaises(transport.LocalTransportError) as caught:
                    runner.complete(request(pin))
                self.assertTrue(caught.exception.dispatched)
                self.assertEqual(len(daemon.generations), 1)

    def test_truncated_and_duplicate_json_bodies_are_refused(self):
        for raw, extra in ((b'{"done":true,"done":true}', 0), (b'{}', 10), (b'{"x":NaN}', 0)):
            with self.subTest(raw=raw), FakeDaemon() as daemon:
                def override(handler):
                    if handler.path != "/api/generate": return False
                    handler.send_response(200)
                    handler.send_header("Content-Length", str(len(raw) + extra))
                    handler.end_headers()
                    handler.wfile.write(raw)
                    return True
                runner, pin = self.make(daemon)
                daemon.override = override
                with self.assertRaises(transport.LocalTransportError):
                    runner.complete(request(pin))
                self.assertEqual(len(daemon.generations), 1)

    def test_header_trickle_cannot_reset_absolute_deadline(self):
        with FakeDaemon() as daemon:
            runner, pin = self.make(daemon, timeout=0.12)
            def override(handler):
                if handler.path != "/api/generate": return False
                try:
                    for byte in b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}":
                        handler.connection.sendall(bytes([byte]))
                        time.sleep(0.025)
                except OSError:
                    pass
                return True
            daemon.override = override
            start = time.monotonic()
            with self.assertRaises(transport.LocalTransportError) as caught:
                runner.complete(request(pin))
            self.assertTrue(caught.exception.dispatched)
            self.assertLess(time.monotonic() - start, 1.0)
            self.assertEqual(len(daemon.generations), 1)

    def test_interruption_preserves_maybe_dispatched_marker(self):
        with FakeDaemon() as daemon:
            runner, pin = self.make(daemon)
            original = runner.client.request
            def interrupted(path, body=None):
                if path == "/api/generate":
                    runner.client.last_generation_started = True
                    raise KeyboardInterrupt()
                return original(path, body)
            with patch.object(runner.client, "request", side_effect=interrupted), self.assertRaises(KeyboardInterrupt):
                runner.complete(request(pin))
            self.assertTrue(runner.client.last_generation_started)


if __name__ == "__main__":
    unittest.main()
