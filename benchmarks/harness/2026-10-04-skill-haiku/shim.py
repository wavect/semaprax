"""Loopback shim: Ollama-shaped /api/tags + /api/generate forwarding to `claude -p --model haiku`.
Used only for the HP-13 matched reuse-skill trial; records every call (no prompt text) to calls.jsonl."""
import json, subprocess, sys, time, hashlib
from http.server import BaseHTTPRequestHandler, HTTPServer
CLAUDE = "/Users/kevin/.local/bin/claude"
CWD = "/private/tmp/claude-501/hp-tools/haiku-shim/cwd"
LOG = "/private/tmp/claude-501/hp-tools/haiku-shim/calls.jsonl"
MODEL = "claude-haiku-4-5"
class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def _send(self, obj):
        b = json.dumps(obj).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        if self.path == "/api/tags":
            return self._send({"models": [{"name": MODEL, "digest": "remote:anthropic/claude-haiku-4-5 via claude CLI 2.1.289 (loopback shim)"}]})
        self.send_response(404); self.end_headers()
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        prompt = body["prompt"]; t = time.time()
        p = subprocess.run([CLAUDE, "-p", prompt, "--model", "haiku", "--tools", "", "--setting-sources", "project",
                            "--max-turns", "1", "--output-format", "json",
                            "--system-prompt", "You complete programming requests. Reply with code only."],
                           cwd=CWD, capture_output=True, text=True, timeout=180)
        try:
            out = json.loads(p.stdout); text = out.get("result", ""); cost = out.get("total_cost_usd")
        except Exception:
            text, cost = "", None
        with open(LOG, "a") as f:
            f.write(json.dumps({"rc": p.returncode, "ms": int((time.time()-t)*1000), "cost_usd": cost,
                                "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
                                "answer_sha256": hashlib.sha256(text.encode()).hexdigest()}) + "\n")
        self._send({"model": MODEL, "response": text, "done": True})
    def log_message(self, *a): pass
HTTPServer(("127.0.0.1", 11500), H).serve_forever()
