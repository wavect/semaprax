"""Runs the Cloudflare adapter with http.client.HTTPSConnection replaced by a fake.

CF_FAKE_MODE selects the canned response, CF_FAKE_LOG receives one JSON line per
captured request. No socket is ever opened. Test-only; never shipped.
"""

import http.client
import json
import os
import runpy
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
FIX = os.path.join(HERE, "fixtures")
MODE = os.environ.get("CF_FAKE_MODE", "ok")
LOG = os.environ["CF_FAKE_LOG"]
ECHO = os.environ.get("SEMAPRAX_HARNESS_SECRET_CLOUDFLARE", "")


def fixture(name):
    with open(os.path.join(FIX, name)) as f:
        d = json.load(f)
    d.pop("_provenance", None)
    return d


def canned(body):
    qid = next(iter(body["questions"]))
    opts = list(body["questions"][qid]["criteria"])
    env = fixture("doc_envelope_success.json")
    res = env["result"]
    res["model"] = body["model"]
    ans = res["answers"].pop("__QID__")
    n = len(opts)
    probs = {o: round(0.1 / (n - 1), 6) for o in opts[:-1]}
    probs[opts[-1]] = round(1 - sum(probs.values()), 6)
    ans["probabilities"], ans["choice"] = probs, opts[-1]
    res["answers"][qid] = ans
    if MODE == "foreign_qid":
        res["answers"] = {"route-0000000000000000": ans}
    elif MODE == "extra_qid":
        res["answers"]["other"] = ans
    elif MODE == "wrong_variant":
        res["model"] = "clef-flash" if body["model"] == "clef" else "clef"
    elif MODE == "no_model":
        del res["model"]
    elif MODE == "bad_sum":
        ans["probabilities"] = {o: 0.9 for o in opts}
    elif MODE == "missing_option":
        ans["probabilities"].pop(opts[0])
    elif MODE == "range":
        ans["probabilities"][opts[0]] = 1.5
    elif MODE == "abstain":
        ans["abstention"] = "abstained"
    elif MODE == "bare_result":
        return 200, res
    elif MODE == "success_with_errors":
        env["errors"] = [{"code": 1, "message": "x"}]
    if MODE == "wrong_choice":
        ans["choice"] = opts[0]
    return 200, env


class Resp:
    def __init__(self, status, raw):
        self.status, self._raw = status, raw

    def read(self, n=-1):
        out, self._raw = self._raw[:n], self._raw[n:]
        return out


class FakeConn:
    def __init__(self, host, port=None, timeout=None, context=None):
        self.host, self.port, self.sock = host, port, None

    def request(self, method, path, body=None, headers=None):
        rec = {"host": self.host, "port": self.port, "method": method, "path": path,
               "headers": dict(headers or {}), "body": json.loads(body) if body else None}
        with open(LOG, "a") as f:
            f.write(json.dumps(rec) + "\n")
        self.body = rec["body"]

    def getresponse(self):
        if MODE == "http401":
            return Resp(401, json.dumps({"errors": [{"message": "bad token " + ECHO}]}).encode())
        if MODE == "http403":
            return Resp(403, b"{}")
        if MODE == "http429":
            return Resp(429, ("slow down " + ECHO).encode())
        if MODE == "http500":
            return Resp(500, ("boom " + ECHO).encode())
        if MODE == "redirect":
            return Resp(302, b"")
        if MODE == "error_envelope":
            return Resp(200, json.dumps(fixture("doc_envelope_error.json")).encode())
        if MODE == "not_json":
            return Resp(200, b"<html>" + ECHO.encode())
        if MODE == "transport":
            raise OSError("connection reset " + ECHO)
        if MODE == "oversize":
            return Resp(200, b"x" * 200000)
        code, doc = canned(self.body)
        return Resp(code, json.dumps(doc).encode())

    def close(self):
        pass


http.client.HTTPSConnection = FakeConn
http.client.HTTPConnection = FakeConn
adapter = os.path.join(HERE, "..", "cloudflare-clef-hosted", "adapter.py")
sys.argv = [adapter]
runpy.run_path(adapter, run_name="__main__")
