"""Backend interface for decision adapters and the Jev/Laya implementations (MR-15).

The shared runtime (systemone_runtime.py) owns the bounded HTTP exchange, the
SystemOne codec and the frame loop. Everything vendor-shaped lives behind this
small interface, supplied by composition: an adapter entrypoint constructs a
backend and passes it to `Config`. The runtime never inspects a backend's name.

A backend provides:
  secret_env / default_model / default_endpoint   config and secret rules
  default_profile()                               derived model profile (data)
  validate_config(cfg)                            refuse before any network use
  endpoint_policy(cfg)                            (scheme, host, port, remote)
  discover(ctx)                                   optional discovery/entitlement
  path / envelope(...)                            request path and envelope
  check_response(info, cfg)                       response identity binding
  call_identity(info, cfg)                        requested/answering identity facts
  billing, confidence_kind, native_threshold_field  typed result metadata
A non-HTTP backend (a local scorer) overrides `send` and ignores the HTTP bits.
"""

import hashlib
import json
import time
import urllib.parse

import model_profile
from systemone_codec import CodecError, SUPPORTED_LANGUAGE

LOOPBACK = ("127.0.0.1", "localhost", "::1")


def is_loopback(host):
    return host in LOOPBACK


def parse_endpoint(endpoint):
    """Validated (url parts, port) for scheme://host[:port]; raises CodecError."""
    if not endpoint:
        raise CodecError("refused", "SPX-HPK002", "SEMAPRAX_HARNESS_ENDPOINT is required (a user-selected existing server)")
    try:
        u = urllib.parse.urlsplit(endpoint)
        port = u.port
    except ValueError:
        raise CodecError("refused", "SPX-HPK002", "endpoint is not a valid URL")
    if u.scheme not in ("http", "https") or not u.hostname or u.username or u.password or u.query or u.fragment or u.path not in ("", "/"):
        raise CodecError("refused", "SPX-HPK002", "endpoint must be scheme://host[:port] without credentials or path")
    return u, port


class Backend:
    """Base backend: no vendor knowledge, conservative defaults."""

    name = "backend"
    secret_env = None
    default_model = None
    default_endpoint = None
    path = "/v1/systemone"
    billing = "unknown"
    confidence_kind = None          # label for native_confidence, e.g. "laya.confidence"
    native_threshold_field = None   # upstream field for SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE

    def __init__(self, env):
        self.secret = (env.get(self.secret_env) if self.secret_env else None) or None

    # -- config and policy -------------------------------------------------
    def default_profile(self, env):
        return {
            "profile_id": f"{self.name}-default", "model": env.get("SEMAPRAX_HARNESS_MODEL") or self.default_model or "unset",
            "checkpoint": None, "identity_kind": "unknown", "score_kind": "option_distribution", "scoreless": False,
            "max_options": 16, "max_state_bytes": 4096, "modalities": ["text"],
        }

    def validate_config(self, cfg):
        """Raise CodecError for a missing secret/model before any network use."""

    def endpoint_policy(self, cfg):
        u, port = parse_endpoint(cfg.endpoint)
        loop = is_loopback(u.hostname)
        return u.scheme, u.hostname, port or (443 if u.scheme == "https" else 80), not loop

    # -- upstream ----------------------------------------------------------
    def discover(self, ctx):
        """Optional discovery/entitlement; `ctx.get(path, limit)` -> (status, bytes)."""

    def envelope(self, body, cfg, native_min):
        """Add backend fields (model, extras) to the codec's SystemOne body."""
        body["model"] = cfg.profile["model"]
        return body

    def send(self, ctx, wire):
        """Default transport: POST the exact bytes to `path`."""
        return ctx.post(self.path, wire)

    # -- response identity -------------------------------------------------
    def check_response(self, info, cfg):
        """Raise CodecError if the upstream answered as a different identity."""

    def call_identity(self, info, cfg):
        """(answering_model, checkpoint, identity_kind) for the typed call record."""
        return info.get("model"), None, "unknown"


class DiscoveryCache:
    """Bounded in-process entitlement cache for one adapter process (MR-12).

    A key is (endpoint, credential identity, model, profile digest): the
    credential identity is a truncated SHA-256 of the secret, never token
    material, and a changed endpoint, key, model or profile is a different key.
    Entries expire after `ttl_s` and are dropped on any 401/403 or entitlement
    refusal. Only a successful listing that names the model is stored; the
    listing says nothing about immutable weights.
    """

    def __init__(self, ttl_s=300.0, max_entries=8, clock=time.monotonic):
        self.ttl, self.max, self.clock = ttl_s, max_entries, clock
        self.entries = {}
        self.lookups = 0

    @staticmethod
    def key(endpoint, secret, model, profile):
        cred = hashlib.sha256(secret.encode()).hexdigest()[:16] if secret else ""
        conf = hashlib.sha256(json.dumps(profile, sort_keys=True).encode()).hexdigest()[:16]
        return (endpoint or "", cred, model, conf)

    def valid(self, key):
        exp = self.entries.get(key)
        if exp is None:
            return False
        if self.clock() >= exp:
            del self.entries[key]
            return False
        return True

    def store(self, key):
        while len(self.entries) >= self.max:
            del self.entries[min(self.entries, key=self.entries.get)]
        self.entries[key] = self.clock() + self.ttl

    def invalidate(self, cred=None):
        """Drop every entry (or every entry of one credential identity)."""
        for k in [k for k in self.entries if cred is None or k[1] == cred]:
            del self.entries[k]


class JevBackend(Backend):
    """Hosted TypeSafe API: bearer key, model entitlement via GET /v1/models."""

    name = "jev"
    secret_env = "SEMAPRAX_HARNESS_SECRET_JEV"
    default_endpoint = "https://api.typesafe.ai"
    billing = "api"
    confidence_kind = "jev.confidence"
    MODELS_MAX = 65536

    def __init__(self, env):
        super().__init__(env)
        self.discovery = DiscoveryCache()
        self.last_discovery = None
        self._key = None

    def default_profile(self, env):
        p = super().default_profile(env)
        p.update(identity_kind="mutable_service", profile_id="jev-" + (p["model"]))
        p["profile_id"] = p["profile_id"][:64]
        return p

    def validate_config(self, cfg):
        if not self.secret:
            raise CodecError("refused", "SPX-HPK003", "SEMAPRAX_HARNESS_SECRET_JEV is not provided by the host")
        if not cfg.env.get("SEMAPRAX_HARNESS_MODEL") and not cfg.env.get(model_profile.ENV):
            raise CodecError("refused", "SPX-HPK014", "SEMAPRAX_HARNESS_MODEL (or a model profile) must name an entitled model")

    def endpoint_policy(self, cfg):
        u, port = parse_endpoint(cfg.endpoint)
        loop = is_loopback(u.hostname)
        if not loop:
            if not cfg.approved:
                raise CodecError("refused", "SPX-HPK001", "remote use needs host approval (SEMAPRAX_HARNESS_REMOTE_APPROVED=1)")
            if u.scheme != "https":
                raise CodecError("refused", "SPX-HPK002", "a remote endpoint must use https")
        return u.scheme, u.hostname, port or (443 if u.scheme == "https" else 80), not loop

    def discover(self, ctx):
        """Entitlement check off the repeated hot path: a fresh listing is
        reused until it expires or an auth/entitlement refusal drops it."""
        import systemone_codec as codec
        key = self.discovery.key(ctx.cfg.endpoint, self.secret, ctx.cfg.profile["model"], ctx.cfg.profile)
        self._key = key
        if self.discovery.valid(key):
            self.last_discovery = "cached"
            return
        self.last_discovery = "fresh"
        self.discovery.lookups += 1
        code, raw = ctx.get("/v1/models", self.MODELS_MAX)
        if code != 200:
            if code in (401, 403):
                self.discovery.invalidate(key[1])
            raise ctx.status_error(code)
        doc = codec.parse_json(raw, self.MODELS_MAX)
        names = [m.get("name") for m in (doc.get("models") if isinstance(doc, dict) else None) or [] if isinstance(m, dict)]
        if ctx.cfg.profile["model"] not in names:
            self.discovery.invalidate(key[1])
            raise CodecError("refused", "SPX-HPK014", "configured model is not listed for this account")
        self.discovery.store(key)

    def send(self, ctx, wire):
        """One inference POST. An auth/entitlement refusal drops the cached
        listing for this credential and is returned as-is: it is never retried
        here and never followed by a hidden rediscovery."""
        code, raw = ctx.post(self.path, wire)
        if code in (401, 403) and self._key is not None:
            self.discovery.invalidate(self._key[1])
        return code, raw

    def call_identity(self, info, cfg):
        p = cfg.profile
        answering = info.get("model")
        # Only a profile-declared pin that the service echoes back is attestable.
        if p["identity_kind"] == "immutable_checkpoint" and p["checkpoint"] and answering == p["checkpoint"] \
                and not p["model"].endswith("latest"):
            return answering, answering, "immutable_checkpoint"
        return answering, None, "mutable_service"


class LayaBackend(Backend):
    """Local Laya server on loopback; the routing checkpoint is checked."""

    name = "laya"
    secret_env = "SEMAPRAX_HARNESS_SECRET_LAYA"
    default_model = "multilingual"
    billing = "local"
    confidence_kind = "laya.confidence"
    native_threshold_field = "min_confidence"

    def default_profile(self, env):
        p = super().default_profile(env)
        p.update(profile_id="laya-" + p["model"][:58], checkpoint=p["model"], identity_kind="local_declared")
        return p

    def endpoint_policy(self, cfg):
        u, port = parse_endpoint(cfg.endpoint)
        if not is_loopback(u.hostname) or u.scheme != "http":
            raise CodecError("refused", "SPX-HPK002", "laya-local only talks to an http loopback server")
        return "http", u.hostname, port or 80, False

    def envelope(self, body, cfg, native_min):
        body = super().envelope(body, cfg, native_min)
        body["lang"] = SUPPORTED_LANGUAGE
        if native_min is not None:
            body[self.native_threshold_field] = native_min
        return body

    def expected_checkpoint(self, cfg):
        return cfg.profile["checkpoint"] or cfg.profile["model"]

    def check_response(self, info, cfg):
        if info.get("checkpoint") not in (None, self.expected_checkpoint(cfg)):
            raise CodecError("refused", "SPX-HPK008", "server answered with a different checkpoint than requested")

    def call_identity(self, info, cfg):
        if info.get("checkpoint") == self.expected_checkpoint(cfg):
            return info.get("model"), info["checkpoint"], "local_declared"
        return info.get("model"), None, "unknown"
