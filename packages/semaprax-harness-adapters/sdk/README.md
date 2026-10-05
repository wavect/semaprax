# Harness adapter SDK (decision adapters)

Python helpers (standard library only) in `python/`:

- `semaprax_harness_adapter.py`: `serve` (simple loop) and `serve_cancellable`
  (worker thread; honors cancel and deadline, refuses oversize results,
  scrubs secrets from diagnostics, reports unexpected errors by type only).
- `decision_fixtures.py`: typed `model-route/v1` and `v2` request builders and
  host-side result validators (`validate_v1_result`, `validate_v2_result`).
  It also defines the `rendered.digest` canonicalization.
- `decision_conformance.py`: `conformance_case(target)` returns a
  `unittest.TestCase` covering wrong question/candidate/identity, abstention,
  timeout, cancellation, crash, excessive output, secret redaction, over-limit
  requests, unsupported modality, scoreless honesty and v1 compatibility.

Copyable starter: `../examples/decision-adapter-starter/` (a deterministic
keyword scorer, deliberately not SystemOne, with a scoreless variant).

Run all: `python3 -m unittest discover -s packages/semaprax-harness-adapters/systemone/tests`
(runs jev, laya, starter and scoreless-starter conformance) and
`python3 -m unittest discover -s packages/semaprax-harness-adapters/sdk/python`.

## Recipe: adding a decision model

Profile-only (an existing backend can serve the model): set
`SEMAPRAX_HARNESS_MODEL_PROFILE` (see `systemone/README.md`) with the new
`model`/`checkpoint`/limits, keep credentials in the usual secret variables,
approve the grant, then qualify the model independently. No adapter code
changes; the profile is validated before any inference.

New protocol or inference mechanism: write an out-of-tree adapter.
1. Copy `examples/decision-adapter-starter/`; replace `score_options` and keep
   request validation, bounds and the v2 result shape.
2. Write `harness-provider.json` (list `decision.evaluate` version 1 and/or 2;
   v2 for typed calls and scoreless results) and adopt it through the normal
   descriptor path; never edit the bundled asset list.
3. Add a `Target` (see `conformance_target.py`) and run `conformance_case`.
4. Optionally run the host qualification for outcome evidence.
