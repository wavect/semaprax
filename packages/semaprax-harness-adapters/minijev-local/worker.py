#!/usr/bin/env python3
"""Warm local Mini Jev closed-choice scoring worker (EXPERIMENTAL).

Started explicitly by the user, owned by the user, stopped by SIGINT/SIGTERM. It
loads ONE already-provisioned model once, then serves single closed-choice
scoring calls on a loopback TCP port. It never downloads: the model must already
be in the local Hugging Face cache (HF_HUB_OFFLINE=1 is set before any import)
and the upstream code must be a checkout at the pinned commit.

  SEMAPRAX_HARNESS_SECRET_MINIJEV=<token>  python3 worker.py serve \
      --minijev-dir /path/to/mini-jev --model Qwen/Qwen3-0.6B --revision <40-hex> --device cpu

Wire: newline-delimited JSON over 127.0.0.1. Line 1 {"auth":token}. Then
{"v":1,"op":"ping"} -> identity, or {"v":1,"op":"score","k":N,"user":text} ->
the upstream scoring result. The worker bounds request size, tokenized prompt
length, option count and queue depth, and runs one forward pass at a time.
`--engine fake` serves the counting fake engine for tests (identity engine "fake").
"""

import argparse
import hashlib
import hmac
import json
import os
import select
import signal
import socket
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import minijev_wire as w  # noqa: E402


def _rss_mb():
    try:
        import resource
        v = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        return round(v / (1024 * 1024) if sys.platform == "darwin" else v / 1024, 1)
    except Exception:
        return None


class Worker:
    """Loopback server around one engine. `engine.identity` and `engine.score_closed_choice(user, k)`."""

    def __init__(self, engine, token, host="127.0.0.1", port=0, max_queue=4, max_user_bytes=w.MAX_USER_BYTES):
        if host not in ("127.0.0.1", "::1", "localhost"):
            raise ValueError("the worker binds loopback only")
        if not token or len(token) < 16:
            raise ValueError("a worker token of at least 16 characters is required")
        self.engine, self.token, self.max_user_bytes = engine, token.encode(), max_user_bytes
        self.admission = threading.BoundedSemaphore(1 + max_queue)   # 1 running + max_queue waiting
        self.run_lock = threading.Lock()                              # concurrency 1: one forward pass at a time
        self.sock = socket.create_server((host, port))
        self.sock.settimeout(0.2)
        self.address = self.sock.getsockname()[:2]
        self.stop_event = threading.Event()
        self.served = 0

    # -- lifecycle ----------------------------------------------------------
    def serve_forever(self):
        threads = []
        while not self.stop_event.is_set():
            try:
                conn, _ = self.sock.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            t = threading.Thread(target=self._connection, args=(conn,), daemon=True)
            t.start()
            threads.append(t)
        self.sock.close()
        for t in threads:
            t.join(timeout=5)

    def start_background(self):
        t = threading.Thread(target=self.serve_forever, daemon=True)
        t.start()
        return t

    def stop(self):
        self.stop_event.set()

    # -- protocol -----------------------------------------------------------
    @staticmethod
    def _send(conn, obj):
        conn.sendall(w.dumps(obj) + b"\n")

    @staticmethod
    def _client_gone(conn):
        try:
            r, _, _ = select.select([conn], [], [], 0)
            return bool(r) and conn.recv(1, socket.MSG_PEEK) == b""
        except OSError:
            return True

    def _connection(self, conn):
        conn.settimeout(30)
        buf = b""
        try:
            authed = False
            while True:
                while b"\n" not in buf:
                    if len(buf) > w.MAX_REQUEST_LINE:
                        self._send(conn, {"ok": False, "code": "too_long", "message": "request line too long"})
                        return
                    chunk = conn.recv(4096)
                    if not chunk:
                        return
                    buf += chunk
                line, buf = buf.split(b"\n", 1)
                if len(line) > w.MAX_REQUEST_LINE:
                    self._send(conn, {"ok": False, "code": "too_long", "message": "request line too long"})
                    return
                try:
                    msg = json.loads(line)
                except ValueError:
                    self._send(conn, {"ok": False, "code": "bad_request", "message": "not JSON"})
                    return
                if not authed:
                    tok = msg.get("auth") if isinstance(msg, dict) else None
                    if not isinstance(tok, str) or not hmac.compare_digest(tok.encode(), self.token):
                        self._send(conn, {"ok": False, "code": "auth", "message": "authentication failed"})
                        return
                    authed = True
                    continue
                self._send(conn, self._handle(conn, msg))
        except (OSError, socket.timeout):
            return
        finally:
            conn.close()

    def _handle(self, conn, msg):
        if not isinstance(msg, dict) or msg.get("v") != w.WIRE_VERSION:
            return {"ok": False, "code": "bad_request", "message": "unsupported wire version"}
        op = msg.get("op")
        if op == "ping":
            return {"ok": True, "identity": self.engine.identity}
        if op != "score":
            return {"ok": False, "code": "bad_request", "message": "unknown op"}
        user, k = msg.get("user"), msg.get("k")
        if not isinstance(k, int) or isinstance(k, bool) or not w.MIN_OPTIONS <= k <= w.MAX_OPTIONS:
            return {"ok": False, "code": "unsupported", "message": f"k must be {w.MIN_OPTIONS}..{w.MAX_OPTIONS}"}
        if not isinstance(user, str) or not user:
            return {"ok": False, "code": "bad_request", "message": "user must be a non-empty string"}
        if len(user.encode()) > self.max_user_bytes:   # bounded BEFORE tokenization
            return {"ok": False, "code": "too_long", "message": "prompt exceeds the byte bound"}
        if not self.admission.acquire(blocking=False):
            return {"ok": False, "code": "busy", "message": "queue is full"}
        try:
            with self.run_lock:
                if self._client_gone(conn):    # cancelled while queued: do not spend a forward pass
                    return {"ok": False, "code": "cancelled", "message": "client went away"}
                warm = self.served > 0
                t0 = time.perf_counter()
                try:
                    out = self.engine.score_closed_choice(user, k)
                except ValueError as err:
                    code = "too_long" if str(err) == "too_long" else "unsupported"
                    return {"ok": False, "code": code, "message": code}
                except Exception as err:   # an engine fault is one refused call, never a dead worker
                    return {"ok": False, "code": "internal", "message": type(err).__name__}
                self.served += 1
                out = dict(out, ok=True, identity=self.engine.identity, warm=warm,
                           latency_ms=round((time.perf_counter() - t0) * 1000, 2), rss_mb=_rss_mb())
                return out
        finally:
            self.admission.release()


# -- the real engine --------------------------------------------------------------
class RealEngine:
    """Wraps r-ms/mini-jev: one prefill, fp32 candidate logits over bare letter tokens, no decoding."""

    def __init__(self, minijev_dir, model, revision, device, dtype, max_prompt_tokens):
        os.environ["HF_HUB_OFFLINE"] = "1"          # never download weights or tokenizer
        os.environ["TRANSFORMERS_OFFLINE"] = "1"
        import subprocess
        head = subprocess.run(["git", "-C", minijev_dir, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
        if head != w.PINNED_CODE_COMMIT:
            raise SystemExit(f"mini-jev checkout is at {head or 'unknown'}, expected {w.PINNED_CODE_COMMIT}")
        if subprocess.run(["git", "-C", minijev_dir, "status", "--porcelain", "--untracked-files=no"], capture_output=True, text=True).stdout.strip():
            raise SystemExit("mini-jev checkout has local modifications; refusing to claim the pinned commit")
        sys.path.insert(0, minijev_dir)
        import torch
        from minijev import letters as L, prompts
        from minijev.engine import Engine
        self.torch, self.L, self.prompts = torch, L, prompts
        self.max_prompt_tokens = max_prompt_tokens
        self.eng = Engine(model_id=model, revision=revision, device=device, dtype=dtype)
        try:
            self.bare, self.space = L.build_tables(self.eng.tok)    # asserts single-token letters
        except AssertionError as err:
            raise SystemExit("unsupported tokenizer: " + str(err))
        vocab = json.dumps(sorted(self.eng.tok.get_vocab().items()), separators=(",", ":")).encode()
        tok_sha = hashlib.sha256(vocab + b"\n" + str(getattr(self.eng.tok, "chat_template", "")).encode()).hexdigest()
        self.identity = {
            "engine": "minijev", "model": model, "revision": revision, "tokenizer_sha": tok_sha,
            "code_commit": head, "system_sha": hashlib.sha256(prompts.SYSTEM_B.encode()).hexdigest(),
            "letters_sha": hashlib.sha256(json.dumps([self.bare[:w.MAX_OPTIONS], self.space[:w.MAX_OPTIONS]]).encode()).hexdigest(),
            "dtype": dtype, "device": device}

    def score_closed_choice(self, user, k):
        torch, L = self.torch, self.L
        prompt = self.prompts.render(self.eng.tok, self.prompts.SYSTEM_B, user)
        n = len(self.eng.tok(prompt, add_special_tokens=False)["input_ids"])
        if n > self.max_prompt_tokens:
            raise ValueError("too_long")
        h, _, _, _ = self.eng.last_hidden([prompt])
        cand = L.candidate_logits_fp32(h, self.eng.embed_weight, self.bare[:k])[0]
        sc = L.score(cand, L.candidate_logits_fp32(h, self.eng.embed_weight, self.space[:k])[0])
        return {"logits": [float(x) for x in cand], "p_cand": sc["p_cand"], "pred_pos": sc["pred_pos"],
                "tie": sc["tie"], "gap": sc["gap"], "prompt_tokens": n}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("serve")
    s.add_argument("--engine", choices=("minijev", "fake"), default="minijev")
    s.add_argument("--minijev-dir")
    s.add_argument("--model", default="Qwen/Qwen3-0.6B")
    s.add_argument("--revision")
    s.add_argument("--device", default="cpu")
    s.add_argument("--dtype", default="float32", choices=("float32", "bfloat16"))
    s.add_argument("--host", default="127.0.0.1")
    s.add_argument("--port", type=int, default=0)
    s.add_argument("--max-queue", type=int, default=4)
    s.add_argument("--max-prompt-tokens", type=int, default=2048)
    a = ap.parse_args(argv)
    token = os.environ.get("SEMAPRAX_HARNESS_SECRET_MINIJEV", "")
    t0 = time.perf_counter()
    if a.engine == "fake":
        from fake_engine import FakeEngine
        engine = FakeEngine()
    else:
        if not a.minijev_dir or not a.revision:
            ap.error("--minijev-dir and --revision (a full 40-hex model revision) are required")
        engine = RealEngine(a.minijev_dir, a.model, a.revision, a.device, a.dtype, a.max_prompt_tokens)
    if not w.valid_identity(engine.identity):
        raise SystemExit("engine identity is incomplete (revision must be a full 40-hex commit)")
    load_s = time.perf_counter() - t0
    srv = Worker(engine, token, a.host, a.port, a.max_queue)
    signal.signal(signal.SIGTERM, lambda *_: srv.stop())
    signal.signal(signal.SIGINT, lambda *_: srv.stop())
    print(f"LISTENING {srv.address[0]}:{srv.address[1]} load_s={load_s:.2f} rss_mb={_rss_mb()}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
