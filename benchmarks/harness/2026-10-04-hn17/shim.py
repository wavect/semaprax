"""Loopback shim for the HN-17 campaign: an Ollama-shaped /api/generate in front of
`claude -p --model haiku` (Claude Code CLI). Hard USD cap ledger (independent of the
harness's own ledger): refuses before sending, counts the CLI's total_cost_usd.
Records every call (no prompt or answer text) to the calls log.

usage: python3 shim.py --port 11500 --ledger LEDGER.json --calls CALLS.jsonl --cap 15 --cwd EMPTY_DIR
"""
import argparse, hashlib, json, os, subprocess, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ap = argparse.ArgumentParser()
ap.add_argument("--port", type=int, default=11500)
ap.add_argument("--ledger", required=True)
ap.add_argument("--calls", required=True)
ap.add_argument("--cap", type=float, default=15.0)
ap.add_argument("--cwd", required=True)
ap.add_argument("--claude", default="/Users/kevin/.local/bin/claude")
ap.add_argument("--min-ceiling", type=float, default=0.03)
A = ap.parse_args()
MODEL = "claude-haiku-4-5"
SYSTEM = "You are a code-writing assistant. Follow the requested output format exactly."
LOCK = threading.Lock()
try:
    ST = json.load(open(A.ledger))
except Exception:
    ST = {"spent_usd": 0.0, "calls": 0, "refused_calls": 0, "max_call_usd": 0.0}
RESERVED = [0.0]


def save():
    ST["cap_usd"] = A.cap
    json.dump(ST, open(A.ledger, "w"), indent=1)


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def _send(self, obj):
        b = json.dumps(obj).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        if self.path == "/api/tags":
            return self._send({"models": [{"name": MODEL, "digest": "remote:anthropic/claude-haiku-4-5 via Claude Code CLI (loopback shim)"}]})
        self.send_response(404)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        prompt = body["prompt"]
        with LOCK:
            ceil = max(ST["max_call_usd"] * 2, A.min_ceiling)
            if ST["spent_usd"] + RESERVED[0] + ceil > A.cap:
                ST["refused_calls"] += 1
                save()
                return self._send({"refused": True, "reason": f"shim spend cap USD {A.cap} would be crossed (spent {ST['spent_usd']:.4f})"})
            RESERVED[0] += ceil
        t0 = time.time()
        text, cost, usage, rc, err = "", None, {}, None, None
        try:
            p = subprocess.run([A.claude, "-p", "--model", "haiku", "--tools", "", "--setting-sources", "project",
                                "--max-turns", "1", "--output-format", "json", "--system-prompt", SYSTEM,
                                "--no-session-persistence", "--disable-slash-commands"],
                               input=prompt, cwd=A.cwd, capture_output=True, text=True, timeout=240,
                               env={**os.environ, "MAX_THINKING_TOKENS": "0"})
            rc = p.returncode
            out = json.loads(p.stdout)
            text, cost, usage = out.get("result", "") or "", out.get("total_cost_usd"), out.get("usage", {}) or {}
            if out.get("is_error") or rc != 0:
                err = "provider-error: " + (text or "")[:120]
        except Exception as e:
            err = f"{type(e).__name__}: {e}"
        with LOCK:
            RESERVED[0] -= ceil
            charged = cost if cost is not None else ceil  # unknown cost is charged the whole ceiling
            ST["spent_usd"] = round(ST["spent_usd"] + charged, 6)
            ST["calls"] += 1
            ST["max_call_usd"] = max(ST["max_call_usd"], charged)
            save()
            with open(A.calls, "a") as f:
                f.write(json.dumps({"rc": rc, "ms": int((time.time() - t0) * 1000), "cost_usd": cost, "charged_usd": charged, "usage": usage,
                                    "error": err, "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
                                    "answer_sha256": hashlib.sha256(text.encode()).hexdigest()}) + "\n")
        if err:
            return self._send({"error": err})
        self._send({"model": MODEL, "response": text, "done": True, "cost_usd": cost,
                    "usage": {"input_tokens": usage.get("input_tokens"), "output_tokens": usage.get("output_tokens"),
                              "cache_read_input_tokens": usage.get("cache_read_input_tokens"),
                              "cache_creation_input_tokens": usage.get("cache_creation_input_tokens")}})

    def log_message(self, *a):
        pass


ThreadingHTTPServer(("127.0.0.1", A.port), H).serve_forever()
