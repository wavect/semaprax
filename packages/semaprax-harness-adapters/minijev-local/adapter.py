#!/usr/bin/env python3
"""Mini Jev local closed-choice decision.evaluate v2/v3 adapter (EXPERIMENTAL).

v3 is the finite runtime choice task choice-select/v1 (MR-11). Mini Jev's
support is finite choice only (letter scoring over the host-rendered options),
not upstream score/noul parity.

Talks to ONE explicitly started warm worker (worker.py) on loopback. It never
starts, installs, downloads or restarts anything: a missing worker is
`unavailable`. See docs/HARNESS-MINIJEV-V1.md.

Environment (host-provided only):
  SEMAPRAX_HARNESS_CFG_ADDR                127.0.0.1:<port>   (loopback only; descriptor config `addr`,
                                           legacy SEMAPRAX_HARNESS_MINIJEV_ADDR also read)
  SEMAPRAX_HARNESS_SECRET_MINIJEV          worker token
  SEMAPRAX_HARNESS_MODEL_PROFILE           optional profile; its checkpoint pins the full identity
  SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE   optional [0,1] abstention threshold on choice_confidence
  SEMAPRAX_HARNESS_MINIJEV_ALLOW_FAKE=1    tests only: accept the counting fake engine
"""

import json
import os
import socket
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "sdk", "python"))
sys.path.insert(0, os.path.join(HERE, "..", "systemone"))
from semaprax_harness_adapter import AdapterError, serve_cancellable  # noqa: E402
import minijev_wire as w  # noqa: E402
import model_profile  # noqa: E402
import systemone_codec as codec  # noqa: E402

PROVIDER_ID = "org.r-ms/minijev-local"
ADAPTER_VERSION = "0.1.0"
SECRET_ENV = "SEMAPRAX_HARNESS_SECRET_MINIJEV"
ACCEPTED = [{"kind": "decision.evaluate", "version": v, "operations": ["evaluate"]} for v in (2, 3)]
PROVENANCE = {"provider_id": PROVIDER_ID, "adapter_version": ADAPTER_VERSION, "upstream_version": "mini-jev@" + w.PINNED_CODE_COMMIT}
SCORE_TOLERANCE = 1e-3
STATUS_BY_WORKER_CODE = {
    "busy": ("unavailable", "SPX-HPK016"), "cancelled": ("refused", "SPX-HPK013"), "too_long": ("refused", "SPX-HPK005"),
    "unsupported": ("unsupported", "SPX-HPK004"), "auth": ("refused", "SPX-HPK012"), "bad_request": ("refused", "SPX-HPK006"),
}


def _err(status, code, message):
    return AdapterError(status, code, message)


class Link:
    """One bounded, cancellable, deadline-bound exchange with the worker over one connection."""

    def __init__(self, addr, token, cancelled, deadline):
        self.cancelled, self.deadline, self.buf = cancelled, deadline, b""
        host, _, port = (addr or "").rpartition(":")
        if host not in ("127.0.0.1", "::1", "localhost") or not port.isdigit():
            raise _err("refused", "SPX-HPK002", "SEMAPRAX_HARNESS_MINIJEV_ADDR must be a loopback host:port")
        self._check()
        try:
            self.sock = socket.create_connection((host, int(port)), timeout=max(min(self.deadline - time.monotonic(), 2.0), 0.01))
        except OSError:
            raise _err("unavailable", "SPX-HPK016", "the Mini Jev worker is not running (the adapter never starts or downloads one)")
        self.sock.settimeout(0.05)
        self.send_line(w.dumps({"auth": token}))

    def _check(self):
        if self.cancelled.is_set():
            raise _err("refused", "SPX-HPK013", "cancelled")
        if time.monotonic() >= self.deadline:
            raise _err("failed", "SPX-HPK011", "deadline exhausted")

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass

    def send_line(self, data):
        try:
            self.sock.sendall(data + b"\n")
        except OSError:
            raise _err("unavailable", "SPX-HPK016", "the Mini Jev worker went away")

    def recv_line(self):
        while b"\n" not in self.buf:
            self._check()
            if len(self.buf) > w.MAX_RESPONSE_LINE:
                raise _err("refused", "SPX-HPK007", f"worker response exceeds {w.MAX_RESPONSE_LINE} bytes")
            try:
                chunk = self.sock.recv(4096)
            except socket.timeout:
                continue
            except OSError:
                raise _err("unavailable", "SPX-HPK016", "the Mini Jev worker went away")
            if not chunk:
                raise _err("unavailable", "SPX-HPK016", "the Mini Jev worker closed the connection")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        if len(line) > w.MAX_RESPONSE_LINE:
            raise _err("refused", "SPX-HPK007", f"worker response exceeds {w.MAX_RESPONSE_LINE} bytes")
        try:
            doc = json.loads(line)
        except ValueError:
            raise _err("refused", "SPX-HPK009", "malformed worker response")
        if not isinstance(doc, dict) or not isinstance(doc.get("ok"), bool):
            raise _err("refused", "SPX-HPK009", "malformed worker response")
        if not doc["ok"]:
            status, code = STATUS_BY_WORKER_CODE.get(doc.get("code"), ("failed", "SPX-HPK012"))
            raise _err(status, code, "worker refused: " + str(doc.get("code"))[:40])
        return doc


def load_profile(env, identity):
    """Profile from the host, else derived from the worker identity (never from a network)."""
    def derive():
        return {"profile_id": ("minijev-" + identity["model"].replace("/", "-"))[:64], "model": identity["model"],
                "checkpoint": w.composite_checkpoint(identity), "identity_kind": "local_declared",
                "score_kind": "candidate_relative", "scoreless": False, "max_options": w.MAX_OPTIONS,
                "max_state_bytes": 4096, "modalities": ["text"]}
    try:
        p = model_profile.load(env, derive)
    except codec.CodecError as err:
        raise _err(err.status, err.code, err.message)
    if p["score_kind"] != "candidate_relative" or p["scoreless"] or p["identity_kind"] != "local_declared" or p["max_options"] > w.MAX_OPTIONS:
        raise _err("refused", "SPX-HPK006", "profile must be candidate_relative, scoring, local_declared, max_options<=16 for this adapter")
    return p


def check_identity(identity, profile, env):
    if not w.valid_identity(identity):
        raise _err("refused", "SPX-HPK009", "worker identity is incomplete or malformed")
    if identity["engine"] != "minijev" and env.get("SEMAPRAX_HARNESS_MINIJEV_ALLOW_FAKE") != "1":
        raise _err("unsupported", "SPX-HPK004", "worker is not a Mini Jev engine")
    if identity["code_commit"] != w.PINNED_CODE_COMMIT:
        raise _err("refused", "SPX-HPK009", "worker runs mini-jev code outside this adapter's pinned commit")
    if profile["model"] != identity["model"] or profile["checkpoint"] != w.composite_checkpoint(identity):
        raise _err("refused", "SPX-HPK009", "worker identity (model revision, tokenizer, code or renderer) differs from the pinned profile")


def validate_scores(doc, k):
    """Independent re-derivation: malformed or inconsistent worker scores never become a route."""
    def finite(x, lo, hi):
        return isinstance(x, (int, float)) and not isinstance(x, bool) and x == x and lo <= x <= hi
    logits, p = doc.get("logits"), doc.get("p_cand")
    if not (isinstance(logits, list) and isinstance(p, list) and len(logits) == len(p) == k):
        raise _err("refused", "SPX-HPK009", "worker scores do not cover exactly the options")
    if not all(finite(x, -1e4, 1e4) for x in logits) or not all(finite(x, 0.0, 1.0) for x in p):
        raise _err("refused", "SPX-HPK009", "worker scores are not finite and in range")
    probs = w.softmax([float(x) for x in logits])
    if any(abs(a - b) > SCORE_TOLERANCE for a, b in zip(probs, p)):
        raise _err("refused", "SPX-HPK009", "worker probabilities disagree with its logits")
    pos = doc.get("pred_pos")
    if not isinstance(pos, int) or isinstance(pos, bool) or not 0 <= pos < k or logits[pos] != max(logits):
        raise _err("refused", "SPX-HPK009", "worker prediction is not the logit argmax")
    tie = doc.get("tie")
    if not isinstance(tie, bool) or tie != (logits.count(max(logits)) > 1) or pos != logits.index(max(logits)):
        raise _err("refused", "SPX-HPK009", "worker tie report is inconsistent")
    return probs, pos, tie


def handle(req, cancelled, env=None):
    env = os.environ if env is None else env
    p = req.get("payload")
    task = p.get("task") if isinstance(p, dict) else None
    version = req.get("capability", {}).get("version")
    if (task, version) not in ((codec.TASK_V2, 2), (codec.TASK_CHOICE, codec.CHOICE_VERSION)):
        raise _err("unsupported", "SPX-HPK004",
                   "only model-route/v2 (v2) and choice-select/v1 (v3) are supported (Mini Jev needs the host-rendered options)")
    choice = task == codec.TASK_CHOICE
    try:
        v2 = codec.validate_request_choice(p) if choice else codec.validate_request_v2(p)
    except codec.CodecError as err:
        raise _err(err.status, err.code, err.message)
    options, rendered = v2["options"], v2["rendered"]
    k = len(options)
    # Host JSON objects arrive key-sorted (m10 < m2): compare label keys as a set.
    if len(set(options)) != k or set(rendered["option_labels"]) != set(options):
        raise _err("refused", "SPX-HPK010", "duplicate or foreign options")
    if k < w.MIN_OPTIONS:
        raise _err("unsupported", "SPX-HPK004", "a single admissible option needs no inference (host bypass)")
    if k > w.MAX_OPTIONS:
        raise _err("refused", "SPX-HPK005", "too many options")
    if v2["features"]["input_modalities"] != ["text"]:
        raise _err("unsupported", "SPX-HPK004", "only text input is supported")
    user = w.render_user(rendered, options)
    if len(user.encode()) > w.MAX_USER_BYTES:
        raise _err("refused", "SPX-HPK005", "rendered prompt exceeds the context bound")
    deadline = time.monotonic() + max(req.get("deadline_ms", 1000), 1) / 1000.0
    nm = env.get("SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE")
    try:
        native_min = float(nm) if nm else None
    except ValueError:
        native_min = float("nan")
    if native_min is not None and not 0.0 <= native_min <= 1.0:
        raise _err("refused", "SPX-HPK006", "SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE must be in [0,1]")
    score_line = w.dumps({"v": w.WIRE_VERSION, "op": "score", "k": k, "user": user})
    if len(score_line) > v2["max_wire_bytes"]:
        raise _err("refused", "SPX-HPK005", "upstream request exceeds max_wire_bytes")
    token = env.get(SECRET_ENV, "")
    if len(token) < 16:
        raise _err("refused", "SPX-HPK003", SECRET_ENV + " is not provided by the host")
    link = Link((env.get("SEMAPRAX_HARNESS_CFG_ADDR") or env.get("SEMAPRAX_HARNESS_MINIJEV_ADDR")), token, cancelled, deadline)
    try:
        link.send_line(w.dumps({"v": w.WIRE_VERSION, "op": "ping"}))
        ident = link.recv_line().get("identity")
        profile = load_profile(env, ident) if w.valid_identity(ident) else None
        if profile is None:
            raise _err("refused", "SPX-HPK009", "worker identity is incomplete or malformed")
        check_identity(ident, profile, env)                       # BEFORE any scoring work
        if len(options) > profile["max_options"] or len(rendered["state"].encode()) > profile["max_state_bytes"]:
            raise _err("refused", "SPX-HPK005", "request exceeds the model profile limits")
        if not choice and rendered["renderer"] != profile["renderer"]:
            raise _err("unsupported", "SPX-HPK004", "renderer is outside the model profile")
        link.send_line(score_line)
        doc = link.recv_line()
    finally:
        link.close()
    if doc.get("identity") != ident:
        raise _err("refused", "SPX-HPK009", "worker identity changed during the call")
    probs, pos, tie = validate_scores(doc, k)
    conf = w.choice_confidence(probs)
    below = native_min is not None and conf < native_min
    abstain = tie or below
    pt = doc.get("prompt_tokens")
    out = {
        "choice": None if abstain else options[pos], "abstain": abstain, "abstention_reason": "native" if abstain else "none",
        "scores": {o: round(x, 6) for o, x in zip(options, probs)}, "score_kind": "candidate_relative",
        "native_confidence": round(conf, 6), "native_confidence_kind": w.CONFIDENCE_KIND, "calibration_id": None,
        "call": {
            "adapter": f"{PROVIDER_ID}@{ADAPTER_VERSION}", "requested_model": profile["model"], "answering_model": ident["model"],
            "checkpoint": w.composite_checkpoint(ident), "identity_kind": "local_declared", "rendered_digest": rendered["digest"],
            "wire_bytes": len(score_line),
            "usage": {"input_tokens": pt if isinstance(pt, int) and not isinstance(pt, bool) and pt >= 0 else None,
                      "output_tokens": 0, "basis": "local_measured"},
            "billing": "local",
        },
    }
    parts = [f"profile={profile['profile_id']}", f"latency_ms={doc.get('latency_ms')}", f"warm={doc.get('warm')}",
             f"rss_mb={doc.get('rss_mb')}", f"device={ident['device']}", f"dtype={ident['dtype']}",
             f"tie={tie}", "experimental=true"]
    return "complete", out, [{"code": "SPX-HPK100", "message": "; ".join(parts)}]


if __name__ == "__main__":
    serve_cancellable(ACCEPTED, {("decision.evaluate", "evaluate"): handle}, PROVENANCE, secret_env=(SECRET_ENV,))
