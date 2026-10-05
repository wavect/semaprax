"""Contract tests for the Cloudflare-hosted Clef / Clef-Flash adapter (MR-05).

Run: python3 -m unittest discover -s packages/semaprax-harness-adapters/systemone/tests
No socket is opened: cf_fake_runner.py replaces http.client.HTTPSConnection.
Fixtures under fixtures/ are documentation-derived, not live captures.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "..", "sdk", "python"))
sys.path.insert(0, ROOT)
import decision_fixtures as fx  # noqa: E402
import systemone_codec as codec  # noqa: E402

SECRET = "cf-token-SECRET-0123456789"
ACCOUNT = "0123456789abcdef0123456789abcdef"
ROUTES = {"@cf/cloudflare/clef": "clef", "@cf/cloudflare/clef-flash": "clef-flash"}


def fixture(name):
    with open(os.path.join(HERE, "fixtures", name)) as f:
        return json.load(f)


class Run:
    def __init__(self, test, mode="ok", model="@cf/cloudflare/clef", drop=(), **extra):
        fd, self.log = tempfile.mkstemp()
        os.close(fd)
        test.addCleanup(os.unlink, self.log)
        env = {"PATH": os.environ.get("PATH", ""), "CF_FAKE_MODE": mode, "CF_FAKE_LOG": self.log,
               "SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID": ACCOUNT, "SEMAPRAX_HARNESS_SECRET_CLOUDFLARE": SECRET,
               "SEMAPRAX_HARNESS_REMOTE_APPROVED": "1", "SEMAPRAX_HARNESS_MODEL": model}
        env.update(extra)
        for k in drop:
            env.pop(k, None)
        self.p = subprocess.Popen([sys.executable, os.path.join(HERE, "cf_fake_runner.py")], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
        self.send({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
            "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": v} for v in (1, 2)]}})
        self.read()

    def send(self, obj):
        self.p.stdin.write(json.dumps(obj).encode() + b"\n")
        self.p.stdin.flush()

    def read(self):
        return json.loads(self.p.stdout.readline())

    def invoke(self, req):
        self.send({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req})
        res = self.read()["result"]
        self.send({"jsonrpc": "2.0", "id": 9, "method": "harness/shutdown"})
        self.read()
        self.out, self.err = self.p.communicate(timeout=10)
        assert SECRET.encode() not in self.out + self.err, "secret leaked to stdout/stderr"
        assert SECRET not in json.dumps(res), "secret leaked into the result"
        return res

    def requests(self):
        with open(self.log) as f:
            return [json.loads(x) for x in f if x.strip()]


class CloudflareContract(unittest.TestCase):
    def go(self, mode="ok", req=None, **kw):
        r = Run(self, mode, **kw)
        return r.invoke(req or fx.v2_request()), r

    def refused(self, res, code, r=None, sent=None):
        self.assertIn(res["status"], ("refused", "failed", "unsupported"))
        self.assertIsNone(res["payload"])
        self.assertEqual(res["diagnostics"][0]["code"], code)
        if sent is not None:
            self.assertEqual(len(r.requests()), sent)

    def test_both_variants_route_and_selector(self):
        for route, sel in ROUTES.items():
            req = fx.v2_request()
            res, r = self.go("ok", req, model=route)
            self.assertEqual(res["status"], "complete", res)
            fx.validate_v2_result(res["payload"], req["payload"])
            (rec,) = r.requests()
            self.assertEqual((rec["host"], rec["port"], rec["method"]), ("api.cloudflare.com", 443, "POST"))
            self.assertEqual(rec["path"], f"/client/v4/accounts/{ACCOUNT}/ai/run/{route}")
            self.assertEqual(rec["body"]["model"], sel)
            self.assertEqual(rec["headers"]["Authorization"], "Bearer " + SECRET)
            self.assertEqual(set(rec["body"]), {"state", "model", "questions"})
            call = res["payload"]["call"]
            self.assertEqual((call["requested_model"], call["answering_model"]), (route, sel))
            self.assertEqual((call["identity_kind"], call["billing"], call["checkpoint"]), ("mutable_service", "api", None))
            self.assertEqual(call["usage"], {"input_tokens": 212, "output_tokens": 3, "basis": "provider_reported"})
            self.assertIn("profile=cf-" + sel, res["diagnostics"][0]["message"])

    def test_v1_request_also_works(self):
        res, r = self.go("ok", fx.v1_request(), model="@cf/cloudflare/clef-flash")
        self.assertEqual(res["status"], "complete", res)
        self.assertEqual(r.requests()[0]["body"]["model"], "clef-flash")

    def test_request_body_matches_documented_shape(self):
        for f in ("doc_request_clef.json", "doc_request_clef_flash.json"):
            doc = fixture(f)
            self.assertIn("_provenance", doc)
            self.assertEqual(set(doc) - {"_provenance", "_route"}, {"model", "state", "questions"})
        _, r = self.go("ok")
        body = r.requests()[0]["body"]
        (q,) = body["questions"].values()
        self.assertEqual(set(q), {"type", "instructions", "criteria"})
        self.assertEqual(q["type"], "choice")
        self.assertEqual(fixture("doc_request_clef.json")["_route"].rsplit("/ai/run/", 1)[1], "@cf/cloudflare/clef")
        self.assertEqual(fixture("doc_request_clef_flash.json")["model"], "clef-flash")

    def test_no_discovery_request(self):
        _, r = self.go("ok")
        self.assertTrue(all(x["method"] == "POST" and "/v1/models" not in x["path"] for x in r.requests()))
        self.assertEqual(len(r.requests()), 1)

    def test_error_envelope_and_status_classes(self):
        res, r = self.go("error_envelope")
        self.refused(res, "SPX-HPK012", r, 1)
        self.assertIn("10000", res["diagnostics"][0]["message"])
        self.assertNotIn("Bearer-SECRET-ECHO", json.dumps(res))
        for mode, status in (("http401", "refused"), ("http403", "refused"), ("http429", "failed"), ("http500", "failed"),
                             ("redirect", "failed"), ("transport", "failed")):
            res, r = self.go(mode)
            self.refused(res, "SPX-HPK012", r, 1)
            self.assertEqual(res["status"], status, mode)
        res, _ = self.go("http429")
        self.assertIn("rate limited", res["diagnostics"][0]["message"])

    def test_malformed_results(self):
        cases = {"foreign_qid": "SPX-HPK008", "extra_qid": "SPX-HPK008", "wrong_variant": "SPX-HPK008", "no_model": "SPX-HPK008",
                 "bad_sum": "SPX-HPK009", "missing_option": "SPX-HPK010", "range": "SPX-HPK009", "wrong_choice": "SPX-HPK010",
                 "not_json": "SPX-HPK008", "bare_result": "SPX-HPK008", "success_with_errors": "SPX-HPK008", "oversize": "SPX-HPK007"}
        for mode, code in cases.items():
            for route in ROUTES:
                res, r = self.go(mode, model=route)
                self.refused(res, code, r, 1)

    def test_native_abstention_is_authoritative(self):
        res, _ = self.go("abstain")
        self.assertEqual(res["status"], "complete")
        self.assertTrue(res["payload"]["abstain"])
        self.assertEqual(res["payload"]["abstention_reason"], "native")

    def test_oversize_state_refused_before_dispatch(self):
        p = fx.v2_payload()
        p["rendered"]["state"] = "x" * 4097
        p["max_wire_bytes"] = 65536
        fx.retag_digest(p)
        req = fx._envelope(p, 2, "inv-000001", 5000, 65536)
        res, r = self.go("ok", req)
        self.refused(res, "SPX-HPK005", r, 0)
        small = fx.v2_request(max_wire_bytes=300)
        res, r = self.go("ok", small)
        self.refused(res, "SPX-HPK005", r, 0)

    def test_image_modality_rejected(self):
        f = dict(fx.V2_FEATURES, input_modalities=["image", "text"])
        res, r = self.go("ok", fx.v2_request(features=f))
        self.refused(res, "SPX-HPK004", r, 0)
        res, r = self.go("ok", model="@cf/cloudflare/clef",
                         SEMAPRAX_HARNESS_MODEL_PROFILE=json.dumps({
                             "profile_id": "cf-img", "model": "@cf/cloudflare/clef", "checkpoint": None,
                             "identity_kind": "mutable_service", "score_kind": "option_distribution", "scoreless": False,
                             "max_options": 16, "max_state_bytes": 4096, "modalities": ["text", "image"]}))
        self.refused(res, "SPX-HPK006", r, 0)

    def test_config_refusals_send_nothing(self):
        cases = [
            ({"drop": ("SEMAPRAX_HARNESS_REMOTE_APPROVED",)}, "SPX-HPK001"),
            ({"drop": ("SEMAPRAX_HARNESS_SECRET_CLOUDFLARE",)}, "SPX-HPK003"),
            ({"drop": ("SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID",)}, "SPX-HPK002"),
            ({"SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID": "../zz"}, "SPX-HPK002"),
            ({"SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID": ACCOUNT[:31]}, "SPX-HPK002"),
            ({"drop": ("SEMAPRAX_HARNESS_MODEL",)}, "SPX-HPK014"),
            ({"model": "@cf/meta/llama"}, "SPX-HPK014"),
            ({"SEMAPRAX_HARNESS_ENDPOINT": "https://evil.example"}, "SPX-HPK002"),
            ({"SEMAPRAX_HARNESS_ENDPOINT": "http://api.cloudflare.com"}, "SPX-HPK002"),
            ({"SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE": "0.5"}, "SPX-HPK006"),
        ]
        for kw, code in cases:
            res, r = self.go("ok", **kw)
            self.refused(res, code, r, 0)

    def test_secret_absent_everywhere(self):
        for mode in ("ok", "http401", "http429", "http500", "error_envelope", "not_json", "transport"):
            _, r = self.go(mode)  # Run.invoke asserts stdout/stderr/result carry no secret
            self.assertNotIn(SECRET.encode(), r.out + r.err)

    def test_descriptor_and_identity_separation(self):
        with open(os.path.join(ROOT, "cloudflare-clef-hosted", "harness-provider.json")) as f:
            d = json.load(f)
        self.assertEqual(d["permissions"]["network"], ["https://api.cloudflare.com"])
        self.assertEqual(d["permissions"]["secrets"], ["SEMAPRAX_HARNESS_SECRET_CLOUDFLARE"])
        self.assertEqual(d["support"]["tested"], [])
        a, _ = self.go("ok", model="@cf/cloudflare/clef")
        b, _ = self.go("ok", model="@cf/cloudflare/clef-flash")
        self.assertNotEqual(a["diagnostics"][0]["message"].split("; ")[1], b["diagnostics"][0]["message"].split("; ")[1])
        self.assertEqual(codec.TASK_V2, "model-route/v2")


if __name__ == "__main__":
    unittest.main()
