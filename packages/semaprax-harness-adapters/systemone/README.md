# SystemOne decision adapters (HP-11)

`laya-local/` and `jev-hosted/` speak `semaprax.harness-rpc.v1` for
`decision.evaluate/v1` (task `model-route/v1`). `systemone_codec.py` and
`systemone_runtime.py` are the shared codec, transport and frame loop; Python 3
standard library only. Contract and gate: [HARNESS-LAYA-JEV-V1](../../../docs/HARNESS-LAYA-JEV-V1.md).

Tests (fake loopback servers, no network, no credentials):
`python3 -m unittest discover -s packages/semaprax-harness-adapters/systemone/tests`

## Backends, profiles and v2 (MR-15, MR-01, MR-02, MR-03)

`systemone_backend.py` defines the small `Backend` interface; `JevBackend` and
`LayaBackend` implement it and each `*/adapter.py` constructs its backend and
passes it to `systemone_runtime.Config`. The runtime owns only the bounded HTTP
exchange, the codec and the frame loop, and never inspects a backend name.
A backend supplies: `secret_env`, `default_model`, `default_endpoint`,
`default_profile(env)`, `validate_config(cfg)`, `endpoint_policy(cfg)`,
`discover(ctx)`, `path`/`envelope(body, cfg, native_min)`/`send(ctx, wire)`,
`check_response(info, cfg)`, `call_identity(info, cfg)`, `billing`,
`confidence_kind` and `native_threshold_field`.

Both adapters list `decision.evaluate` versions 1 and 2. v1 (`model-route/v1`)
is unchanged. v2 (`model-route/v2`) forwards the host-rendered
`instructions`/`state`/`option_labels` verbatim (criteria = option labels),
recomputes `rendered.digest`, refuses a body larger than `max_wire_bytes`
before sending, and returns `score_kind`, `native_confidence` (+kind),
`abstention_reason` and a typed `call` record (identity, usage, billing).

Model profile: `SEMAPRAX_HARNESS_MODEL_PROFILE` is JSON with exactly
`profile_id, model, checkpoint, identity_kind, score_kind, scoreless,
max_options, max_state_bytes, modalities` (optional `renderer`). Credentials
and endpoints are refused as unknown members. Absent, the backend derives a
default from `SEMAPRAX_HARNESS_MODEL`. Two profiles of one adapter select
different models with no code change; configuration is not qualification.

Thresholds: `SEMAPRAX_HARNESS_MIN_SCORE` is a deprecated alias for the
host-side minimum chosen-option mass and is never forwarded upstream.
`SEMAPRAX_HARNESS_NATIVE_MIN_CONFIDENCE` is forwarded to Laya as
`min_confidence` (Jev has none: setting it is refused). Setting both emits an
`SPX-HPK101` warning. Native abstention stays authoritative.

Conformance for every backend: `tests/test_conformance.py` (see the SDK README).
