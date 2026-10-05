"""Backend for a local, explicitly started Clef warm worker (MR-06).

Reuses the Backend interface and SystemOne codec; the worker speaks the
documented `POST /v1/systemone` shape on loopback and a `GET /readyz`
readiness document. Nothing here starts, installs or downloads anything, and
there is no hosted fallback: every failure is unsupported/unavailable/refused.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "systemone"))
import clef_identity as ident  # noqa: E402
import systemone_backend as be  # noqa: E402
import systemone_codec as codec  # noqa: E402
from systemone_codec import CodecError  # noqa: E402

READY_MAX = 8192


class ClefLocalBackend(be.Backend):
    name = "clef-local"
    default_model = "clef-flash"
    billing = "local"
    confidence_kind = "clef.choice_probability"
    native_threshold_field = None

    def __init__(self, env):
        super().__init__(env)
        self.lock_path = env.get(ident.LOCK_ENV) or None
        self.verified = None  # readiness document accepted by the latest discover()

    # -- lock / release ----------------------------------------------------
    def _release(self, cfg):
        model = cfg.profile["model"]
        try:
            rel = ident.release(ident.load_lock(self.lock_path), model)
        except ident.LockError as err:
            raise CodecError("unsupported", "SPX-HPK014", str(err))
        if rel.get("status") != "supported":
            raise CodecError("unsupported", "SPX-HPK014", f"{model} is unavailable unless provisioned and verified (status: {rel.get('status')})")
        return rel

    def default_profile(self, env):
        model = env.get("SEMAPRAX_HARNESS_MODEL") or self.default_model
        try:
            checkpoint = ident.identity_digest(ident.release(ident.load_lock(self.lock_path), model))
        except (ident.LockError, KeyError):
            checkpoint = "sha256:unpinned"
        return {
            "profile_id": ("clef-local-" + model)[:64], "model": model, "checkpoint": checkpoint,
            "identity_kind": "immutable_checkpoint", "score_kind": "option_distribution", "scoreless": False,
            "max_options": 16, "max_state_bytes": 4096, "modalities": ["text"],
        }

    def validate_config(self, cfg):
        rel = self._release(cfg)
        if cfg.profile["checkpoint"] != ident.identity_digest(rel):
            raise CodecError("refused", "SPX-HPK006", "model profile checkpoint does not match the pinned release identity")

    def endpoint_policy(self, cfg):
        u, port = be.parse_endpoint(cfg.endpoint)
        if not be.is_loopback(u.hostname) or u.scheme != "http":
            raise CodecError("refused", "SPX-HPK002", "clef-local only talks to an http loopback worker")
        return "http", u.hostname, port or 80, False

    # -- readiness ---------------------------------------------------------
    def discover(self, ctx):
        cfg = ctx.cfg
        rel = self._release(cfg)
        code, raw = ctx.get("/readyz", READY_MAX)
        if code not in (200, 503):
            raise ctx.status_error(code)
        doc = codec.parse_json(raw, READY_MAX)
        if not isinstance(doc, dict):
            raise CodecError("refused", "SPX-HPK008", "readiness document is not an object")
        state, reason = doc.get("state"), doc.get("reason")
        if state in ("loading", "verifying"):
            raise CodecError("unavailable", "SPX-HPK016", f"worker is cold-starting ({state}); retry when /readyz reports ready")
        if state == "failed":
            status = "unsupported" if isinstance(reason, str) and reason.startswith("unsupported_device") else "unavailable"
            raise CodecError(status, "SPX-HPK016" if status == "unavailable" else "SPX-HPK004", "worker could not become ready: " + str(reason)[:120])
        if state != "ready" or code != 200:
            raise CodecError("unavailable", "SPX-HPK016", "worker is not ready")
        expected = ident.identity_digest(rel)
        if doc.get("profile") != cfg.profile["model"] or doc.get("revision") != rel["revision"] or doc.get("identity") != expected \
                or doc.get("digests") != ident.group_digests(rel):
            raise CodecError("refused", "SPX-HPK008", "worker model identity does not match the pinned release lock")
        if doc.get("digests_verified") is not True:
            raise CodecError("refused", "SPX-HPK008", "worker did not verify the release digests at start")
        if doc.get("joint_head") is not True or doc.get("device") not in rel["devices"]:
            raise CodecError("unsupported", "SPX-HPK004", "worker is not running the joint head on a supported device")
        self.verified = expected

    def envelope(self, body, cfg, native_min):
        body["model"] = cfg.profile["model"]
        return body

    # -- response identity -------------------------------------------------
    def check_response(self, info, cfg):
        if self.verified is None or info.get("checkpoint") != self.verified:
            raise CodecError("refused", "SPX-HPK008", "worker answered under a different identity than the verified release")

    def call_identity(self, info, cfg):
        if self.verified is not None and info.get("checkpoint") == self.verified:
            return info.get("model"), self.verified, "immutable_checkpoint"
        return info.get("model"), None, "unknown"
