#!/usr/bin/env python3
"""command.view/v1 adapter for Caveman input compression (pinned runtime 3.1.0).

The runtime is host-provisioned and adopted explicitly (`adopt --upstream <abs path>`);
this adapter never installs, logs in, fetches or reaches the network. It runs the
upstream executable once per `view` as `<upstream> input-compress` with a JSON request on
stdin and expects one JSON reply on stdout:

  request  {"kind":"command-output","text":"..."}
  reply    {"mode":"compress"|"record","text":"...","runtime":{"bind":"127.0.0.1","telemetry":false}}

Anything but `mode == "compress"`, a loopback bind and telemetry off is refused (the
host then delivers raw). The adapter also refuses a view that is not smaller (bytes) or
that drops an error/fatal line the raw output carried, so the host falls back to raw.
It never runs the user's command and never compresses stderr-less structured output.
"""
import base64
import ipaddress
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

PROVIDER = "ai.caveman/caveman-command-view"
KIND = "command.view"
PINNED_VERSION = "3.1.0"
DEFAULT_MIN_BYTES = 1024
MAX_RAW = 8 * 1024 * 1024
CALL_TIMEOUT = 20
CRITICAL = re.compile(r"(?i)\b(error|fail(ed|ure|ures)?|panic(ked)?|fatal|critical|exception|traceback)\b")
BENIGN = re.compile(r"\.\.\. (ok|ignored)\s*$")
PATCH = re.compile(r"^(diff --git |--- a/|\+\+\+ b/|@@ -\d)", re.M)
IDENTITY = re.compile(r"caveman (\d+\.\d+\.\d+)")


def upstream():
    path = os.environ.get("SEMAPRAX_HARNESS_UPSTREAM", "")
    if not os.path.isabs(path) or not os.path.isfile(path):
        raise AdapterError("unavailable", "caveman-missing", "SEMAPRAX_HARNESS_UPSTREAM is not an absolute path to the Caveman runtime")
    return path


def retention_dir():
    d = os.environ.get("SEMAPRAX_HARNESS_RETENTION_DIR", "")
    if not os.path.isabs(d) or not os.path.isdir(d):
        raise AdapterError("unavailable", "retention-missing", "SEMAPRAX_HARNESS_RETENTION_DIR is not an absolute directory")
    return d


def runtime_env(ret):
    """Closed environment: no ambient config or credentials, telemetry off, loopback only."""
    home = os.path.join(ret, "caveman-home")
    os.makedirs(home, mode=0o700, exist_ok=True)
    return {
        "PATH": "/usr/bin:/bin",
        "HOME": home,
        "CAVEMAN_TELEMETRY": "0",
        "CAVEMAN_TELEMETRY_DISABLED": "1",
        "DO_NOT_TRACK": "1",
        "CAVEMAN_WORK_TAGS": "0",
        "CAVEMAN_BIND": "127.0.0.1",
        "CAVEMAN_OFFLINE": "1",
        "CAVEMAN_NO_LOGIN": "1",
        "CAVEMAN_MODE": "compress",
    }


def probe_identity(exe, env):
    out = subprocess.run([exe, "--version"], capture_output=True, timeout=10, env=env)
    m = IDENTITY.fullmatch(out.stdout.decode("utf-8", "replace").strip())
    if out.returncode != 0 or not m or m.group(1) != PINNED_VERSION:
        raise AdapterError("unavailable", "caveman-version", f"runtime identity is not caveman {PINNED_VERSION}")


def plan(req):
    p = req.get("payload") or {}
    if p.get("external_hooks") or "caveman" in (p.get("lineage") or []):
        return "complete", {"form": "post-execution", "route": "bypass", "reason": "external-owner-holds-the-view"}, []
    cfg = p.get("config") or {}
    est = p.get("estimated_output_bytes")
    if isinstance(est, int) and est < int(cfg.get("min_bytes", DEFAULT_MIN_BYTES)):
        return "complete", {"form": "post-execution", "route": "bypass", "reason": "small-output"}, []
    return "complete", {"form": "post-execution", "route": "post-execution", "operation": "view",
                        "raw_recovery": "host-retained-streams"}, []


def _stream(p, key, ret):
    if p.get(key + "_path"):
        path = os.path.realpath(os.path.join(ret, p[key + "_path"]))
        if os.path.commonpath([path, os.path.realpath(ret)]) != os.path.realpath(ret):
            raise AdapterError("refused", "path-outside-retention", f"{key}_path escapes the retention directory")
        with open(path, "rb") as f:
            return f.read(MAX_RAW + 1)
    if p.get(key + "_b64") is not None:
        return base64.b64decode(p[key + "_b64"])
    return str(p.get(key, "")).encode("utf-8")


def _critical_missing(raw_text, view_text):
    return [s for s in (l.strip() for l in raw_text.splitlines())
            if s and CRITICAL.search(s) and not BENIGN.search(s) and s not in view_text]


def _loopback(bind):
    try:
        return ipaddress.ip_address(bind).is_loopback
    except ValueError:
        return bind == "localhost"


def _unsupported(code):
    return "unsupported", None, [{"code": "caveman.bypass", "message": code}]


def view(req):
    p = req.get("payload") or {}
    exe, ret = upstream(), retention_dir()
    env = runtime_env(ret)
    out_b, err_b = _stream(p, "stdout", ret), _stream(p, "stderr", ret)
    total = len(out_b) + len(err_b)
    if total > MAX_RAW:
        return _unsupported("output-too-large")
    min_bytes = int(p.get("min_bytes", DEFAULT_MIN_BYTES))
    if total < min_bytes:
        return _unsupported("small-output")
    try:
        text = out_b.decode("utf-8") + ("\n[stderr]\n" + err_b.decode("utf-8") if err_b else "")
    except UnicodeDecodeError:
        return _unsupported("not-utf8")
    if "\x00" in text or text.lstrip()[:1] in ("{", "[") or PATCH.search(text):
        return _unsupported("unsupported-format")
    probe_identity(exe, env)
    try:
        proc = subprocess.run([exe, "input-compress"], input=json.dumps({"kind": "command-output", "text": text}).encode(),
                              capture_output=True, timeout=CALL_TIMEOUT, env=env)
    except subprocess.TimeoutExpired:
        raise AdapterError("failed", "caveman-timeout", "runtime exceeded its time limit")
    if proc.returncode != 0:
        raise AdapterError("failed", "caveman-failed", f"runtime exited {proc.returncode}")
    try:
        reply = json.loads(proc.stdout.decode("utf-8"))
        out, mode, rt = reply["text"], reply["mode"], reply["runtime"]
        if not isinstance(out, str) or not isinstance(rt, dict):
            raise ValueError("shape")
    except (ValueError, KeyError, TypeError):
        raise AdapterError("failed", "caveman-invalid-output", "runtime reply is not the pinned input-compress shape")
    if rt.get("telemetry") is not False or not _loopback(str(rt.get("bind", ""))):
        raise AdapterError("refused", "caveman-egress", "runtime reports telemetry or a non-loopback bind")
    if mode != "compress":
        return _unsupported(f"runtime-mode-{mode}")  # record mode changes no payload: no saving to claim
    if len(out.encode("utf-8")) >= len(text.encode("utf-8")):
        return _unsupported("not-smaller")
    missing = _critical_missing(text, out)
    if missing:
        raise AdapterError("failed", "caveman-dropped-critical", f"view dropped {len(missing)} error line(s)")
    v = {"text": out, "lossless": False, "omissions": max(0, text.count("\n") - out.count("\n"))}
    if p.get("recovery_handle"):
        v["recovery_handle"] = p["recovery_handle"]
    return "complete", {"form": "post-execution", "view": v}, [
        {"code": "caveman.compress", "message": f"raw_bytes={total} view_bytes={len(out.encode())}"}]


if __name__ == "__main__":
    serve([{"kind": KIND, "version": 1, "operations": ["plan", "view"]}],
          {(KIND, "plan"): plan, (KIND, "view"): view},
          {"provider_id": PROVIDER, "adapter_version": "0.1.0", "upstream_version": PINNED_VERSION})
