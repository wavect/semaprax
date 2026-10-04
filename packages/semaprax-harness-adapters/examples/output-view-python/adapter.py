#!/usr/bin/env python3
"""command.view/v1 post-execution adapter: dedupe repeated lines, always keep error lines."""
import base64
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

CRITICAL = re.compile(r"error|fail|panic", re.I)


def _text(p, key):
    if p.get(key + "_path"):
        ret = os.path.realpath(os.environ.get("SEMAPRAX_HARNESS_RETENTION_DIR", ""))
        path = os.path.realpath(os.path.join(ret, p[key + "_path"]))
        if not ret or os.path.commonpath([path, ret]) != ret:
            raise AdapterError("refused", "path-outside-retention", f"{key}_path escapes the retention directory")
        with open(path, "rb") as f:
            return f.read().decode("utf-8", "replace")
    if p.get(key + "_b64") is not None:
        return base64.b64decode(p[key + "_b64"]).decode("utf-8", "replace")
    return str(p.get(key, ""))


def view(stdout, stderr, max_bytes):
    """Return (text, lossless, omissions). Critical lines are never dropped."""
    out, seen, omissions = [], {}, 0
    for stream, text in (("stdout", stdout), ("stderr", stderr)):
        lines = text.split("\n")
        if lines and lines[-1] == "":
            lines.pop()
        for line in lines:
            if CRITICAL.search(line):
                out.append(line)
            elif line in seen:
                seen[line] += 1
                omissions += 1
            else:
                seen[line] = 1
                out.append(line)
    # Annotate collapsed repeats, then enforce the byte budget by dropping non-critical lines from the end.
    def render(lines):
        return "\n".join(
            f"{l} (x{seen[l]})" if seen.get(l, 1) > 1 and not CRITICAL.search(l) else l for l in lines)
    text = render(out)
    i = len(out)
    while len(text.encode()) > max_bytes and i > 0:
        i -= 1
        if not CRITICAL.search(out[i]):
            del out[i]
            omissions += 1
            text = render(out)
    return text, omissions == 0, omissions


def handle(req):
    p = req.get("payload") or {}
    max_bytes = p.get("max_bytes", 4096)
    if not isinstance(max_bytes, int) or max_bytes < 0:
        raise AdapterError("refused", "bad-max-bytes", "max_bytes must be a non-negative integer")
    text, lossless, omissions = view(_text(p, "stdout"), _text(p, "stderr"), max_bytes)
    return "complete", {"form": "post-execution", "view": {"text": text, "lossless": lossless, "omissions": omissions}}, []


if __name__ == "__main__":
    serve([{"kind": "command.view", "version": 1, "operations": ["view"]}],
          {("command.view", "view"): handle},
          {"provider_id": "org.example/output-view", "adapter_version": "0.1.0", "upstream_version": "builtin-0.1.0"})
