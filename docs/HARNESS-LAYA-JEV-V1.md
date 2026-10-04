# Laya and Jev decision adapters v1 (HP-11)

Owner: `packages/semaprax-harness-adapters/systemone/`. Contract:
`decision.evaluate/v1` task `model-route/v1` in
[HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md) and
[HARNESS-DECISION-V1](HARNESS-DECISION-V1.md). Diagnostics letter `K`.
Status: adapters are **opt-in and experimental**. The measured gate for
default enablement is defined below and is **not passed**; rules stay default.

## Sources read and pins

| Thing | Pin | Source read |
| --- | --- | --- |
| Laya package | `laya==0.3.26` (PyPI name `laya`, Apache-2.0), tag `v0.3.26` = commit `f585201011c32ba2dec2f9e7279daa8baf59318a` | `docs/http-api.md`, `README.md`, `pyproject.toml`, `laya/router.py`, `laya/agent.py`, `laya/common.py`, `laya/serve.py` at that tag (GitHub API, read only) |
| Laya checkpoint used | `convaiinnovations/laya` @ `7b928d828b7b0e022f929d9bd2e44165aa270148`, subfolder `multilingual` (mmBERT-base, 322M params, 1024 ctx) | Hugging Face model API and `multilingual/rl_agent_config.json` |
| Checkpoint files | `multilingual/model.safetensors` 643,835,514 B sha256 `9d628fd971b700382ac6f65920a86f149777b2e748e0c955fb3b19695aa8f204`; `multilingual/tokenizer/tokenizer.json` 34,363,188 B sha256 `609d8f4c067cd3950f88594c5a802616cea245823836ef5848ee4fc40aab5b6f` (both re-hashed locally after download and equal to the Hub's LFS digests) | same |
| Other checkpoints (not used) | English `model.safetensors` 842,609,210 B; `typed-decisions` 842,609,220 B | same |
| Jev API | TypeSafe API `info.version` `0.2.0`, `https://api.typesafe.ai/openapi.json` (sha256 of the document read: `a191f8a7df6bd6fedced8120dd0fd106f88575d1d1c8360d08900a6c7c0360d5`), rendered at `https://api.typesafe.ai/redoc` | the OpenAPI document |

Laya's own HTTP route is documented as a Jev-compatible `/v1/systemone`; the
shared codec relies only on the fields both documents state.

## Wire protocol (documented fields only)

`POST /v1/systemone`, JSON.

Request (both): `state` (text or JSON), `questions` (object keyed by a name we
choose; each `{type: "choice", instructions, criteria: {<choice>: <description>}}`).
Jev additionally requires `model` (name or alias from `GET /v1/models`).
Laya also accepts optional `model` (checkpoint name), `lang`, `min_confidence`.

Response (both): `model`, `answers` keyed by the question names, `usage
{input_tokens, output_tokens}`. A `choice` answer carries `choice`,
`probabilities` (per option, 0..1, about summing to 1) and `confidence`. Laya
adds `answer_confidence` (probability of the answer), `action`, `routing`
(`model`, `repo`, `reason`, `detection`), and, when `min_confidence` is sent,
`abstention` (`passed|abstained|unevaluated`).

Jev auth: `Authorization: Bearer <API_KEY>`; `GET /v1/models` lists the
models and aliases available to the authenticated account
(`{models: [{name, description, release_date}]}`); the example alias in the
document is `jev-latest`. Laya auth is optional (`LAYA_API_KEY`).

Laya limits relevant here (docs/http-api.md): body 2 MiB, `state` 50,000
characters, 100 options per `choice` (HTTP guard), 16 concurrent requests;
errors 400/401/413/422/500/503. Options share a 256-token head budget on
`laya-multilingual`; the model card advises under about 20 options.

## Mapping a model-route/v1 decision onto a typed choice

| model-route/v1 | SystemOne |
| --- | --- |
| `options` (logical model ids from the host catalog) | `criteria` keys; the description is the id itself (a choice "is interpreted by its name alone", Jev) |
| `features` (closed set: task_family, estimated_context_tokens, requires_structured_output, requires_tools, confidentiality, latency_class) | `state`: fixed-order `key: value` lines, at most 512 bytes. No prompt, source or path is ever sent. |
| invocation identity | question name `route-<sha256(invocation_id, task, options)[:16]>`. The response must contain exactly that answer name (the binding echo). |
| result `scores` | the answer's `probabilities`, one per option, nothing else |
| result `choice` | the answer's `choice`, which must be an option and the (tolerance 1e-3) argmax |
| `abstain` | Laya `abstention == "abstained"` or `low_confidence`, or chosen score below host-set `SEMAPRAX_HARNESS_MIN_SCORE` |

The model never sees model capabilities, prices or catalog descriptions, only
the ids. A score is probability mass over id labels, **not** a measured chance
that the model will succeed.

## Adapter configuration (host-provided environment only)

| Variable | laya-local | jev-hosted |
| --- | --- | --- |
| `SEMAPRAX_HARNESS_ENDPOINT` | required, `http://` loopback only (`127.0.0.1`, `localhost`, `::1`), a server the user already runs | optional, default `https://api.typesafe.ai`; non-loopback needs https |
| `SEMAPRAX_HARNESS_SECRET_JEV` | n/a | required; sent only in `Authorization`; never logged |
| `SEMAPRAX_HARNESS_SECRET_LAYA` | optional bearer for a `LAYA_API_KEY` server | n/a |
| `SEMAPRAX_HARNESS_MODEL` | default `multilingual` | required; must be listed by `GET /v1/models` (entitlement check) |
| `SEMAPRAX_HARNESS_REMOTE_APPROVED` | n/a | must be `1` for any non-loopback endpoint |
| `SEMAPRAX_HARNESS_MIN_SCORE` | optional abstain threshold in [0,1] | same |

Descriptors: `laya-local/harness-provider.json` (`ai.convai/laya-decision`,
network `loopback:user-selected-endpoint`, no secrets) and
`jev-hosted/harness-provider.json` (`ai.typesafe/jev-decision`, network
`https://api.typesafe.ai`, secret `SEMAPRAX_HARNESS_SECRET_JEV`,
`support.tested` empty). Permissions are requests; grants stay in the host's
trust store. A jev alias ending in `latest` is reported as
`model_is_mutable_alias=true`; the answering model name from the response is
recorded in the `SPX-HPK100` diagnostic and no immutability is assumed.

The adapter never starts a server, installs a package or downloads a
checkpoint. Absent server: `unavailable` (`SPX-HPK016`); the decision layer
then falls back to rules.

## Refusals (none produces an accepted route)

| Code | Status | Cause |
| --- | --- | --- |
| HPK001 | refused | remote use without host approval |
| HPK002 | refused | endpoint missing, not loopback (laya), not https when remote, or malformed |
| HPK003 | refused | Jev key not provided by host |
| HPK004 | unsupported/refused | unsupported task, input language (`language` other than `en`), or feature profile (any member outside the closed set, value outside its set) |
| HPK005 | refused | overlong: more than 16 options, more than 640 option-id bytes, rendered features over 512 bytes, or an overlong feature value. Never truncated; nothing is sent. |
| HPK006 | refused | missing mandatory feature, malformed options or threshold |
| HPK007 | refused | response over `min(budget.max_result_bytes, 65536)` |
| HPK008 | refused | invalid JSON, missing/foreign answer name (binding mismatch), wrong answer type, different Laya checkpoint than requested |
| HPK009 | refused | NaN/Infinity, probability outside [0,1], or sum not about 1 |
| HPK010 | refused | choice or probability key outside the options, choice not the argmax |
| HPK011 | failed | deadline exceeded |
| HPK012 | refused/failed | HTTP 400/401/403/404/413/422 refused; 429/5xx failed. The response body is never echoed. |
| HPK013 | refused | `harness/cancel` received (the live socket is shut down) |
| HPK014 | refused | Jev model unset or not entitled |
| HPK016 | unavailable | connection refused |
| HPK099 | failed | unexpected adapter error (type name only) |

Secrets: a configured secret is scrubbed from every emitted string; the host
environment is the only source; stdout carries protocol frames only; stderr is
silent. The contract tests assert the key never appears in stdout, stderr or
result envelopes, including when the fake server echoes it in an error body.

## Laya limitations that bind this integration

From the model cards and README (read at the pins above):

- Base checkpoints are not zero-shot decision engines: typed-decisions accuracy
  0.362 (English) and 0.342 to 0.352 (multilingual) against a 0.461 majority
  baseline; gains come from fine-tuning on a task's own data.
- Shipped uncalibrated: `temperature = [1.0, 1.0, 1.0]`; confidence over-confident
  (mean ECE 0.314 for multilingual before per-(type, option-count) refit, 0.106 after).
- `confidence` (entropy-based) and `answer_confidence` differ and neither
  matches Jev's formula; thresholds do not transfer between Laya and Jev.
- Its Router chooses among its own three checkpoints by script/language. It is
  **not** a router among arbitrary coding LLMs; this integration uses the typed
  choice primitive over logical model ids instead.
- Keep `choice` under about 20 options; ordinal `score` and `noul` are weaker.
- Shared wire shape implies neither equal calibration nor equal behavior.

## Real local Laya execution (macOS arm64, 2026-10-04)

Footprint measured before running anything (PyPI wheel sizes for the resolved
set were about 130 MB; installed size unknown until installed): venv
**754 MiB** (python 3.12.12, `laya[serve]==0.3.26`, resolved on this host to
torch 2.14.1, transformers 5.18.0, huggingface_hub 1.33.0) plus checkpoint
**647 MiB** on disk = **1.40 GiB**, under the 1.5 GiB budget. The smallest
checkpoint is `multilingual` (the English and typed-decisions checkpoints are
842 MB each). Free disk stayed above 5 GiB.

Preparation was explicit and separate from the adapter: one
`snapshot_download` of the pinned revision with `allow_patterns` restricted to
`multilingual/*` into a private cache. The server ran with `HF_HUB_OFFLINE=1`,
`LAYA_MODELS=multilingual`, `LAYA_PRELOAD=0` (lazy), `LAYA_DEVICE=cpu`,
`LAYA_MAX_LOADED=1`, `LAYA_REVISION=<pin>`, bound to `127.0.0.1:18427`;
`/health` reported `loaded: ["multilingual"]` with the pinned revision on `cpu`.

Measured (single machine, CPU, one process, indicative not a benchmark):

| Quantity | Value |
| --- | --- |
| Resident memory before first request | 210 MiB RSS |
| Resident memory after load | about 1.94 GiB RSS (2,032,768 KiB) |
| First request (cold load, through the adapter) | 5.3 s (8.4 s on an earlier raw cold request) |
| Warm request | 41 to 56 ms (mean 44.7 ms, p95 48.8 ms over 12 calls) |

The cold load alone exceeds the default router latency ceiling
(`router_max_latency_ms` 2000), so a lazily started Laya would time out and
fall back to rules on its first decision; a deployment must warm it
explicitly. The provisioned test does exactly that.

Pipeline evidence: `tests/real_tools_v1/decision_local.rs` spawns the
laya-local adapter, drives it over stdio, parses the result with
`ResultEnvelope::parse_for` and runs `decision::decide` with an explicit
(experimental) provider: source `Provider`, `provider_status` `experimental`,
one router call. Its second test points the adapter at a dead port and
asserts the explicit rules fallback (`Fallback(Unavailable)`).

## Evaluation design and default-enablement gate

Defined **before any measured-outcome result exists**. (The pilot below ran
before this text was finalized, but no threshold here was derived from it and
the pilot uses author-prior labels, so it is not evidence.)

Corpus: `crates/semaprax-harness/tests/fixtures/decision_corpus/`.

- Items: model-route/v1 features per task family (mechanical edits, tests/docs,
  localized debugging, semantic/law-sensitive).
- **Labels come from downstream outcome at a fixed budget**: for each logical
  model, run the real task under the same `{max_cost_micros, max_attempts}`
  and record accepted (law gates and tests pass), cost, latency and attempts.
  `best_route` = cheapest accepted model, else strongest. Never another model's
  confidence. This needs real agent task runs, which this lane did not do.
- Splits are by project (`splits.json`): train, calibration and evaluation
  projects are disjoint; calibration fits per-(option-count) temperatures only
  on the calibration split and evaluation never calibrates.
- Arms: rules-only (default), Laya decision, optional Jev decision, and each
  fixed single model as baselines.
- Reported per task profile (task x provider x checkpoint x catalog): accepted
  rate, **total cost per accepted task including router cost**, router latency
  (warm and cold), resident memory, failure/escalation rate, fallback rate,
  reliability diagram and ECE on the held-out split.

**Gate (a learned provider may be auto-enabled for a task profile only if all hold on
held-out projects, with measured labels):**

1. At least 5 held-out projects and 200 evaluated items for that profile.
2. Total cost per accepted task (router cost and escalations included) is at
   least 5 percent below rules-only, with the 95 percent bootstrap interval
   of the saving above zero.
3. Accepted-task rate is not below rules-only by more than 1 percentage point
   (non-inferiority, 95 percent interval).
4. Failure plus escalation rate is not above rules-only by more than 2 points.
5. ECE of the chosen-option score is at most 0.05 on the held-out split after
   calibration fitted on the calibration split alone.
6. Warm p95 router latency is at most 250 ms and within the policy ceiling;
   cold load and resident memory are recorded.
7. The `semantic_law` family is never auto-routed by a learned provider
   (rules route it to the strongest admissible model); law and test acceptance
   are never relaxed.

Passing sets `EnablementGate::Passed { evidence }` for exactly that task and
profile; anything else leaves `NotEvaluated`, a no-go is recorded and kept, and
the adapter stays explicit opt-in (`experimental`).

### Pilot result (seed set, not evidence)

`decision_corpus/seed.jsonl` has 24 items (train 8, calibration 4, eval 12;
projects disjoint) with `label.status = "seed-author-prior"`: the best route is
the author's guess and `outcomes` are `null`. `pilot-results-laya-multilingual.json`
holds the run of `tools/eval_pilot.py` on the 12 eval items with the real
local Laya above (proxy cost units 1/3/10, escalating one step on an
under-route):

| Arm | agrees with author prior | accepted (proxy) | total proxy cost |
| --- | --- | --- | --- |
| rules-only | 5 of 12 | 7 of 12 | 74 |
| Laya multilingual, zero-shot | 1 of 12 | 4 of 12 | 93 |

Laya chose `m-strong` for all three mechanical items and `m-cheap` for the
nine others, including every `semantic_law` item. The 3-bin ECE against the
author prior is 0.373 on 12 chosen answers, which is uninformative at this n.
This is consistent with the model card (near chance zero-shot, uncalibrated).
Jev was not run: no authorized key or hosted smoke test.

**Conclusion: no-go.** Gate item 1 cannot be met (no measured outcomes, 12 seed
items), and the pilot shows the zero-shot checkpoint worse than rules. Rules
remain the default; the Laya and Jev adapters are explicit opt-in.

## What is unverified

- Jev: live availability, entitlement behavior, real response bytes, latency,
  rate limits and `GET /v1/models` contents. The adapter was exercised only
  against a fake that reproduces the OpenAPI 0.2.0 shapes; no credential was
  used and nothing was sent to `api.typesafe.ai`.
- Laya: only `laya-multilingual` on CPU on one macOS arm64 host. The English and
  typed-decisions checkpoints, GPU/MPS, Linux and `LAYA_JEV_STRICT=1` are not
  exercised (strict mode keeps the fields used here, per its documentation).
- No routing-quality claim: no measured-outcome corpus exists, and no
  fine-tuned or calibrated checkpoint was trained or evaluated.
- Whether the host passes these environment variables to adapters is a
  host-side concern (HP-03); the adapters read them and nothing else.
