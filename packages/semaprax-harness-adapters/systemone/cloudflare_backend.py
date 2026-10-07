"""Cloudflare Workers AI transport for the hosted Clef and Clef-Flash routes (MR-05).

One backend, two explicit profiles. The request goes to the account-scoped REST
path `/client/v4/accounts/{account_id}/ai/run/{model}` on api.cloudflare.com,
the response is a Workers AI envelope `{success, errors, messages, result}`
whose `result` is the SystemOne document. No discovery request, no retry, no
redirect, no body echo. See docs/HARNESS-CLOUDFLARE-CLEF-V1.md.
"""

import json
import re

from systemone_backend import Backend
from systemone_codec import CodecError

HOST = "api.cloudflare.com"
ACCOUNT_ENV = "SEMAPRAX_HARNESS_CLOUDFLARE_ACCOUNT_ID"
ACCOUNT_RE = re.compile(r"[0-9a-fA-F]{32}\Z")
# route model (profile.model) -> body `model` selector the endpoint variant requires
VARIANTS = {"@cf/cloudflare/clef": "clef", "@cf/cloudflare/clef-flash": "clef-flash"}
# Service-side state is truncated to a token limit; refuse anything above this
# many bytes (<= 1 token per byte) so truncation can never erase host facts.
MAX_STATE_BYTES = 4096
MAX_ERROR_CODES = 4


class CloudflareBackend(Backend):
    name = "cloudflare-clef"
    secret_env = "SEMAPRAX_HARNESS_SECRET_CLOUDFLARE"
    billing = "api"
    confidence_kind = "cloudflare.clef.confidence"

    def __init__(self, env):
        super().__init__(env)
        self.account = (env.get(ACCOUNT_ENV) or "").strip()

    @staticmethod
    def selector(model):
        return VARIANTS.get(model)

    def route_path(self, cfg):
        return f"/client/v4/accounts/{self.account}/ai/run/" + cfg.profile["model"]

    def default_profile(self, env):
        model = env.get("SEMAPRAX_HARNESS_MODEL") or "unset"
        return {
            "profile_id": "cf-" + (self.selector(model) or "unset"), "model": model, "checkpoint": None,
            "identity_kind": "mutable_service", "score_kind": "option_distribution", "scoreless": False,
            "max_options": 16, "max_state_bytes": MAX_STATE_BYTES, "modalities": ["text"],
        }

    def validate_config(self, cfg):
        if not self.secret:
            raise CodecError("refused", "SPX-HPK003", "SEMAPRAX_HARNESS_SECRET_CLOUDFLARE is not provided by the host")
        if not ACCOUNT_RE.match(self.account):
            raise CodecError("refused", "SPX-HPK002", f"{ACCOUNT_ENV} must be the 32-hex Cloudflare account id")
        p = cfg.profile
        if p["model"] not in VARIANTS:
            raise CodecError("refused", "SPX-HPK014", "model must be @cf/cloudflare/clef or @cf/cloudflare/clef-flash "
                             "(set SEMAPRAX_HARNESS_MODEL or a model profile)")
        if p["modalities"] != ["text"] or p["identity_kind"] != "mutable_service" or p["checkpoint"] is not None \
                or p["scoreless"] or p["score_kind"] != "option_distribution" or p["max_state_bytes"] > MAX_STATE_BYTES:
            raise CodecError("refused", "SPX-HPK006", "profile must be text-only, mutable_service, no checkpoint, "
                             f"option_distribution scores and max_state_bytes <= {MAX_STATE_BYTES}")

    def endpoint_policy(self, cfg):
        if cfg.env.get("SEMAPRAX_HARNESS_ENDPOINT") not in (None, "", "https://" + HOST):
            raise CodecError("refused", "SPX-HPK002", "the Cloudflare transport only talks to https://" + HOST)
        if not cfg.approved:
            raise CodecError("refused", "SPX-HPK001", "remote use needs host approval (SEMAPRAX_HARNESS_REMOTE_APPROVED=1)")
        return "https", HOST, 443, True

    def envelope(self, body, cfg, native_min):
        if len(body["state"].encode()) > MAX_STATE_BYTES:
            raise CodecError("refused", "SPX-HPK005", "state exceeds the bound that the service cannot truncate")
        body["model"] = self.selector(cfg.profile["model"])
        return body

    def send(self, ctx, wire):
        code, raw = ctx.post(self.route_path(ctx.cfg), wire)
        if code == 429:
            raise CodecError("failed", "SPX-HPK012", "rate limited by the service (HTTP 429)")
        if code != 200:
            raise ctx.status_error(code)
        doc = _json(raw)
        if not isinstance(doc, dict) or set(doc) != {"success", "errors", "messages", "result"} \
                or not isinstance(doc["errors"], list) or not isinstance(doc["messages"], list):
            raise CodecError("refused", "SPX-HPK008", "response is not a Workers AI envelope")
        if doc["success"] is not True:
            codes = [e["code"] for e in doc["errors"] if isinstance(e, dict) and isinstance(e.get("code"), int)]
            raise CodecError("failed", "SPX-HPK012", "service reported failure (error codes: %s)" % (codes[:MAX_ERROR_CODES] or "none"))
        if doc["errors"] or not isinstance(doc["result"], dict):
            raise CodecError("refused", "SPX-HPK008", "envelope success without a result object")
        return 200, json.dumps(doc["result"], separators=(",", ":")).encode()

    def check_response(self, info, cfg):
        want = self.selector(cfg.profile["model"])
        got = info.get("model")
        if got not in (want, "@cf/cloudflare/" + want):
            raise CodecError("refused", "SPX-HPK008", "service answered as a different model variant than requested")

    def call_identity(self, info, cfg):
        self.check_response(info, cfg)
        return cfg.profile["model"], None, "mutable_service"


def _json(raw):
    try:
        return json.loads(raw.decode("utf-8"), parse_constant=lambda n: (_ for _ in ()).throw(ValueError(n)))
    except ValueError:
        raise CodecError("refused", "SPX-HPK008", "response is not valid JSON")
