#!/usr/bin/env python3
"""TC-12 benchmark-local model.generate adapter: forwards the decoded prompt text over loopback to
shim.py (an Ollama-shaped front for `claude -p --model haiku`) and returns a typed TC-01 receipt.
Needs only a loopback network permission. Prompt and answer text are never logged. The SDK file is
copied next to this adapter by the campaign setup."""
import base64, json, os, sys, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

URL = "http://127.0.0.1:%s/api/generate" % os.environ.get("TC12_SHIM_PORT", "11512")


def prompt_text(raw):
    """Canonical prompt: JSON {"schema", "text"}. TC-04 `ordered-v1`: the host sends the segment texts
    concatenated as plain (non-JSON) text, which is the prompt as is."""
    try:
        doc = json.loads(raw)
    except ValueError:
        return raw.decode("utf-8")
    if isinstance(doc, dict) and isinstance(doc.get("text"), str):
        return doc["text"]
    return raw.decode("utf-8")


def generate(req):
    p = req["payload"]
    text = prompt_text(base64.b64decode(p["input_base64"]))
    cap = p.get("max_output_tokens")
    body = {"prompt": text}
    if cap:
        body["max_output_tokens"] = cap
    rq = urllib.request.Request(URL, json.dumps(body).encode(), {"Content-Type": "application/json"})
    try:
        r = json.load(urllib.request.urlopen(rq, timeout=280))
    except Exception as e:
        raise AdapterError("unavailable", "TC12-SHIM", "shim unreachable: %s" % type(e).__name__)
    if r.get("refused"):
        raise AdapterError("refused", "TC12-CAP", "shim spend cap refused the call")
    if r.get("error"):
        raise AdapterError("failed", "TC12-PROVIDER", str(r["error"])[:160])
    out = r["response"].encode()
    u = r.get("usage") or {}
    n = lambda k: u.get(k) or 0
    hit_cap = bool(cap) and (r.get("message_output_tokens") or 0) >= cap
    controls = {"max_output_tokens": {"status": "applied", "effective": cap} if cap else {"status": "unsupported"}}
    cost = r.get("cost_usd")
    receipt = {"schema": "semaprax.harness-model-receipt.v1", "protocol": "anthropic_messages",
               "model": r.get("model", "claude-haiku-4-5"),
               "finish_reason": "max_tokens" if hit_cap else "end_turn",
               "usage": {"input_tokens": n("input_tokens"), "cache_read_input_tokens": n("cache_read_input_tokens"),
                         "cache_creation_input_tokens": n("cache_creation_input_tokens"), "output_tokens": n("output_tokens")},
               "controls": controls}
    if cost is not None:
        receipt["provider_cost_micros"] = int(round(cost * 1e6))
    payload = {"model": receipt["model"], "output_base64": base64.b64encode(out).decode(),
               "usage": {"input_bytes": len(p["input_base64"]), "output_bytes": len(out)}, "receipt": receipt}
    return ("partial" if hit_cap else "complete"), payload, []


if __name__ == "__main__":
    serve([{"kind": "model.generate", "version": 1, "operations": ["generate"]}],
          {("model.generate", "generate"): generate},
          {"provider_id": "org.wavect/haiku-cli-shim", "adapter_version": "0.1.0", "upstream_version": "2.1.289"})
