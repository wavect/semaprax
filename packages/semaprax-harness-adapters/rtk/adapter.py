#!/usr/bin/env python3
"""command.view/v1 adapter for rtk-ai/rtk (qualified versions only: see rtk_families.QUALIFIED_VERSIONS).

Operations
  plan  {argv, cwd_rel, estimated_output_bytes?, external_hooks?, lineage?, form?, config?}
        -> route "post-execution" (host runs argv itself, then calls `view`),
           route "wrapped" (explicit opt-in; raw is recoverable only for failures/truncations),
           or route "bypass" with a reason.
  view  {form, argv, one of stdout|stdout_b64|stdout_path and one of stderr|stderr_b64|stderr_path,
         min_bytes?, max_bytes?, recovery_handle?, config?}   (paths are relative to the retention dir)
        -> {form:"post-execution", view:{text, lossless, omissions, recovery_handle?}} via `rtk pipe`.

The adapter never reads PATH/HOME configuration or agent settings, never runs the
user's command, and never writes outside SEMAPRAX_HARNESS_RETENTION_DIR.
"""
import base64
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "sdk", "python"))
sys.path.insert(0, HERE)
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402
from rtk_families import MERGES_STDERR, PINNED_VERSION, Bypass, Unqualified, classify, qualify  # noqa: E402

PROVIDER = "ai.rtk/rtk-command-view"
KIND = "command.view"
DEFAULT_MIN_BYTES = 1024
MAX_RAW = 8 * 1024 * 1024  # rtk pipe refuses > 10 MiB; stay below
CRITICAL = re.compile(r"(?i)\b(error|fail(ed|ure|ures)?|panic(ked)?|fatal|critical|exception|traceback)\b")
BENIGN = re.compile(r"\.\.\. (ok|ignored)\s*$")
PRESERVE_LIMIT = 20
BLOCK_LINES = 60
BLOCK_BYTES = 6000
MAX_BLOCKS = 5
CARGO_BLOCK_START = re.compile(r"^---- .+ ----$")
PYTEST_BLOCK_START = re.compile(r"^_{3,} .+ _{3,}$")
# libtest ends a failure block at the next header or the trailing `failures:` list; pytest at the next rule line.
BLOCK_END = re.compile(r"^(---- .+ ----|failures:|_{3,} .+ _{3,}|={3,} .*)$")


def upstream():
    path = os.environ.get("SEMAPRAX_HARNESS_UPSTREAM", "")
    if not os.path.isabs(path) or not os.path.isfile(path):
        raise AdapterError("unavailable", "rtk-missing", "SEMAPRAX_HARNESS_UPSTREAM is not an absolute path to rtk")
    return path


def retention_dir():
    d = os.environ.get("SEMAPRAX_HARNESS_RETENTION_DIR", "")
    if not os.path.isabs(d) or not os.path.isdir(d):
        raise AdapterError("unavailable", "retention-missing", "SEMAPRAX_HARNESS_RETENTION_DIR is not an absolute directory")
    return d


def rtk_env(ret):
    """Closed environment for every rtk process: no ambient config, no telemetry, state under retention."""
    home = os.path.join(ret, "rtk-home")
    os.makedirs(home, mode=0o700, exist_ok=True)
    return {
        "PATH": "/usr/bin:/bin",
        "HOME": home,
        "RTK_TELEMETRY_DISABLED": "1",
        "RTK_NO_TOML": "1",
        "RTK_DB_PATH": os.path.join(ret, "rtk", "history.db"),
        "RTK_RECALL_DB": os.path.join(ret, "rtk", "recall.db"),
        "RTK_TEE_DIR": os.path.join(ret, "rtk", "tee"),
    }


def probe_identity(rtk):
    """Run `rtk --version` and qualify it against the explicit version table."""
    out = subprocess.run([rtk, "--version"], capture_output=True, timeout=10, env={"PATH": "/usr/bin:/bin"})
    text = out.stdout.decode("utf-8", "replace").strip()
    if out.returncode != 0:
        raise AdapterError("unavailable", "rtk-version", f"`rtk --version` exited {out.returncode}")
    try:
        return qualify(text)
    except Unqualified as u:
        raise AdapterError("unavailable", "rtk-version", f"{u.reason}: {u.detail}")


def plan(req):
    p = req.get("payload") or {}
    cfg = p.get("config") or {}
    argv = p.get("argv")
    try:
        if p.get("external_hooks") or "rtk" in (p.get("lineage") or []):
            raise Bypass("external-hook-owns-rewrite")
        fam = classify(argv)
    except Bypass as b:
        return "complete", {"form": "wrapper", "route": "bypass", "reason": b.reason}, []
    min_bytes = int(cfg.get("min_bytes", DEFAULT_MIN_BYTES))
    est = p.get("estimated_output_bytes")
    if isinstance(est, int) and est < min_bytes:
        return "complete", {"form": "wrapper", "route": "bypass", "reason": "small-output"}, []
    rtk = upstream()
    ret = retention_dir()
    try:
        probe_identity(rtk)  # a version outside the qualified table never transforms
    except AdapterError as e:
        if e.code == "rtk-version":
            return "complete", {"form": "wrapper", "route": "bypass", "reason": "rtk-version-unqualified"}, []
        raise
    wants_wrapper = bool(cfg.get("allow_wrapper")) and (p.get("form") == "wrapper" or fam["filter"] is None)
    can_wrap = wants_wrapper and fam["wrapper"] and "/" not in argv[0]
    if fam["filter"] is not None and not can_wrap:
        return "complete", {
            "form": "post-execution", "route": "post-execution", "operation": "view",
            "family": fam["family"], "filter": fam["filter"],
            "raw_recovery": "host-retained-streams",
        }, []
    if can_wrap:
        env = rtk_env(ret)
        return "complete", {
            "form": "wrapper", "route": "wrapped", "family": fam["family"],
            "argv": [rtk] + argv,
            "env": {k: v for k, v in env.items() if k.startswith("RTK_")},
            "resolves_via": "PATH",
            "recovery": {
                "kind": "recall-db", "dir": ret, "db": env["RTK_RECALL_DB"],
                "coverage": "failure-or-truncation-only",
                "handle_pattern": r"\[(?:full output|\+\d+ hidden): rtk recall ([0-9a-f]{12})\]",
                "retrieve_argv": [rtk, "recall", "<handle>", "--full"],
            },
        }, []
    reason = "wrapper-recovery-incomplete" if fam["wrapper"] else "no-stdin-filter"
    return "complete", {"form": "wrapper", "route": "bypass", "reason": reason}, []


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
    out = []
    for line in raw_text.splitlines():
        s = line.strip()
        if s and CRITICAL.search(s) and not BENIGN.search(s) and s not in view_text:
            out.append(s)
    return out


def _failure_blocks(raw_text, family):
    """Failure detail blocks of libtest (`---- t stdout ----`) or pytest (`___ t ___`) output."""
    start = CARGO_BLOCK_START if family == "cargo-test" else PYTEST_BLOCK_START if family == "pytest" else None
    if start is None:
        return []
    blocks, cur = [], None
    for line in raw_text.splitlines():
        if cur is not None and BLOCK_END.match(line.strip()) and not start.match(line.strip()):
            blocks.append(cur)
            cur = None
        if start.match(line.strip()):
            if cur is not None:
                blocks.append(cur)
            cur = [line]
        elif cur is not None:
            cur.append(line)
    if cur is not None:
        blocks.append(cur)
    return blocks


def _restore_blocks(raw_text, view_text, family):
    """Whole failure blocks whose lines the filter cut (nested/multiline messages), bounded."""
    out, used = [], 0
    for blk in _failure_blocks(raw_text, family):
        while blk and not blk[-1].strip():
            blk.pop()
        if all((l.strip() in view_text) for l in blk if l.strip()):
            continue
        text = "\n".join(blk[:BLOCK_LINES])
        if used + len(text) > BLOCK_BYTES or len(out) >= MAX_BLOCKS:
            break
        used += len(text)
        out.append(text + (f"\n[adapter: {len(blk) - BLOCK_LINES} more lines in recovered raw output]" if len(blk) > BLOCK_LINES else ""))
    return out


def view(req):
    p = req.get("payload") or {}
    try:
        fam = classify(p.get("argv"))
    except Bypass as b:
        return "unsupported", None, [{"code": "rtk.bypass", "message": b.reason}]
    if fam["filter"] is None:
        return "unsupported", None, [{"code": "rtk.bypass", "message": "no-stdin-filter"}]
    rtk = upstream()
    ret = retention_dir()
    probe_identity(rtk)
    out_b, err_b = _stream(p, "stdout", ret), _stream(p, "stderr", ret)
    if len(out_b) + len(err_b) > MAX_RAW:
        return "unsupported", None, [{"code": "rtk.bypass", "message": "output-too-large"}]
    merged = fam["family"] in MERGES_STDERR
    raw_in = out_b + (err_b if merged else b"")
    decoded = raw_in.decode("utf-8", "replace")
    bad = decoded.count("�") - raw_in.decode("utf-8", "ignore").count("�")
    total = len(out_b) + len(err_b)
    min_bytes = int(p.get("min_bytes", (p.get("config") or {}).get("min_bytes", DEFAULT_MIN_BYTES)))
    handle = {"recovery_handle": p["recovery_handle"]} if p.get("recovery_handle") else {}
    diags = []
    if total < min_bytes:
        text = decoded + _stderr_tail(err_b, merged)
        diags.append({"code": "rtk.small-output-bypass", "message": f"raw_bytes={total} min_bytes={min_bytes}"})
        return "complete", _result(text, bad == 0 and _utf8_ok(err_b), bad, handle), diags
    proc = subprocess.run([rtk, "pipe", "-f", fam["filter"]], input=decoded.encode("utf-8"),
                          capture_output=True, timeout=20, env=rtk_env(ret))
    if proc.returncode != 0:
        raise AdapterError("failed", "rtk-pipe-failed", proc.stderr.decode("utf-8", "replace")[:200])
    filtered = proc.stdout.decode("utf-8", "replace")
    omissions = max(0, decoded.count("\n") - filtered.count("\n")) + bad
    text = filtered
    if merged:
        restored = _restore_blocks(decoded, filtered, fam["family"])
        if restored:
            text += "\n[adapter: failure detail the filter cut, restored whole]\n" + "\n".join(restored)
            filtered_for_critical = filtered + "\n" + "\n".join(restored)
            omissions = max(0, omissions - sum(len(r.splitlines()) for r in restored))
        else:
            filtered_for_critical = filtered
        missing = _critical_missing(decoded, filtered_for_critical)
        if missing:
            shown = missing[:PRESERVE_LIMIT]
            text += "\n[adapter: critical lines absent from the compressed view]\n" + "\n".join(shown)
            if len(missing) > len(shown):
                text += f"\n[adapter: {len(missing) - len(shown)} more critical lines in recovered raw output]"
            omissions = max(0, omissions - len(shown))
    else:
        text += _stderr_tail(err_b, merged)
    lossless = filtered == decoded and bad == 0
    diags.append({"code": "rtk.filter", "message": f"filter={fam['filter']} raw_bytes={total} view_bytes={len(text.encode())}"})
    return "complete", _result(text, lossless, omissions, handle), diags


def _utf8_ok(b):
    try:
        b.decode("utf-8")
        return True
    except UnicodeDecodeError:
        return False


def _stderr_tail(err_b, merged):
    if merged or not err_b:
        return ""
    return "\n[stderr]\n" + err_b.decode("utf-8", "replace")


def _result(text, lossless, omissions, handle):
    v = {"text": text, "lossless": bool(lossless), "omissions": int(omissions)}
    v.update(handle)
    return {"form": "post-execution", "view": v}


if __name__ == "__main__":
    serve([{"kind": KIND, "version": 1, "operations": ["plan", "view"]}],
          {(KIND, "plan"): plan, (KIND, "view"): view},
          {"provider_id": PROVIDER, "adapter_version": "0.1.0", "upstream_version": PINNED_VERSION})
