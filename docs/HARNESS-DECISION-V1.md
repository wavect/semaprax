# Harness decision layer v1 (HP-10)

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence only.

Audience: toolchain contributors and harness adapter authors.

Owner: `crates/semaprax-harness/src/decision/`. Contract: `decision.evaluate/v1`
in [HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md). Diagnostics letter `J`.

## Flow

1. Resolve the task in the compile-time registry. Active: `model-route/v1`.
   Reserved and refused: `tool-select/v1`, `context-plan/v1` (`HPJ002`);
   anything else `HPJ001`. A provider cannot add or activate a task.
2. Hard host policy screens the catalog (destination vs confidentiality,
   structured-output/tools capability, context size, cost, latency). `secret`
   is local-only. Nothing admissible: refuse (`HPJ005`).
3. One admissible plan, or a rules-only task family: the rules provider
   (`semaprax/rules-decision`) decides with zero router calls. Otherwise, with
   no enabled router, rules decide too: cheapest admissible, or strongest for
   configured hard families; ties break by id.
4. An enabled router (`DecisionInvoker`) is consulted within the call and
   latency ceilings and the recursion guard. Its payload is validated
   (`HPA040/043/044`), then the choice is revalidated against live
   inputs (`live()`), never trusted. A score is evidence for fallback only and
   grants nothing; the minimum score is the adapter profile's, not a constant.
5. Abstain, unavailable, timeout, out-of-distribution, low confidence, stale,
   rejected choice, invalid result, cap exhausted and recursion fall back to
   rules, or refuse (`HPJ013`) under `fallback: refuse`.
6. The decision is frozen as `FrozenRoutePlan`; `AttemptLedger` meters
   `InitialRoute`, `ReasoningEscalation` and `TransportRetry` against separate
   budgets, idempotently per attempt id. The plan is never reordered.

`FrozenRoutePlan::to_provider_slots()` returns `(id, authorized)` in exact
order and maps 1:1 onto `ProviderPolicy::new(Vec<ProviderSlot>)` (slot 0 is the
primary; failover is forward-only; unauthorized slots are never admitted).

## Digests, cache, replay

Each decision binds feature, catalog, policy (policy plus budget) and candidate
digests (`json::digest`). The cache key is provider/model/checkpoint plus the
feature, catalog and policy digests; it is FIFO-bounded and hits are still
revalidated. `DecisionRecord` replays without a router call; any changed
digest or plan refuses (`HPJ007`).

## Learned providers

Automatic selection requires an `EnablementGate { task, profile, status }` with
`Passed { evidence }` for exactly that task and profile (default
`NotEvaluated`, so rules decide). Explicit selection runs with visible status
`experimental`.

## Diagnostics

HPJ001 unregistered task, 002 reserved task, 003 malformed request/catalog,
004 malformed policy, 005 no admissible model, 007 replay mismatch,
008 malformed record/plan, 009 attempt budget/plan exhausted, 010 attempt id
reused as another kind, 011 choice not admissible, 013 fallback refused,
015 CLI input.

## CLI

`decide <task.json> [--catalog <catalog.json>] [--json]` runs rules and prints
the decision and frozen plan. `task.json`: `{task, features, budget,
catalog?, policy?, lineage_id?}`.

## Request budget (HN-11)

The router's `estimated_context_tokens` is the smallest protected-only request requirement over the catalog, computed by
`workflow::budget` with each model's mapped tokenizer (byte upper bound when unknown); it is never `context_bytes / 4`.
The chosen model is then revalidated against the full serialized request and rerouted or refused before any provider call
(`SPX-HPD100`, see HARNESS-WORKFLOW-V1). A router call is itself budgeted: its serialized request is reserved in the task
ledger (closed-choice output reserve `16 + 8 × options`, see MR-03 below) and recorded as an incurred
`decision` observation.


## Evidence, modes and qualification (HN-16)

One routing engine (`decide`) runs under `governed_decide`; there is no second
router. Modes: `rules` (default), `pin`, `experimental` (learned provider,
visibly experimental) and `qualified-auto`.

- **Pins and remote prohibition.** A project pin wins over every mode and any
  user pin; a pin never consults a router and a pin the policy cannot admit
  refuses (`HPJ016`), never falls back. `user_allow_remote = false` removes
  remote plans from the approved set in every mode.
- **Evidence key.** `EvidenceKey` = task, provider, weights/checkpoint digest,
  approved-catalog digest, feature normalization (`model-route/v1/closed-features.v1`)
  and declared task distribution. Any change is a different key, so evidence
  never transfers. `EvidenceRegistry` stores `EvidenceRecord`s; a record carries
  no authority.
- **Outcomes.** Only independently verified results (verifier named; completion,
  regressions, attempts, usage/cost, latency) at one matched budget. Origin is
  `real`, `fixture` or `unavailable`. An unavailable cell is never a success and
  a success with unknown cost is refused (`HPJ017`); unknown cost is charged the
  matched ceiling in comparisons. Router, context and skill cost and the retry
  owner (host or gateway) are recorded per outcome and counted in total cost.
- **Gate.** `GateSpec` (predeclared; digest recorded) compares rules and the
  learned arm on the sealed eval items: enough items, no overlap with trained
  or calibrated items, arms matched, only `real` cells, completion non-inferior,
  no extra regressions, cost saving after router overhead, latency ratio.
  `gate_for` returns `Passed { evidence: "evidence:<key>:<record>" }` only on go.
  `qualified-auto` also needs a `SessionLock` for exactly the live key; changed
  weights, catalog, normalization or distribution fall back to rules.
- **Shadow.** With `shadow_max_calls > 0` and a provider that is not enabled,
  rules decide and a recommendation is evaluated within that call budget
  (`changes_route: false`); its calls are charged to the same task ledger.
- **Budget.** The workflow reserves router requests in the HN-11 task ledger; when
  the remaining task tokens cannot cover a router request the router is skipped
  and rules decide. `recheck_dispatch` re-screens the chosen model against the
  final serialized request size, privacy and capabilities (`HPJ018`).
  `cheap_bypass_micros` skips the router for cheap candidate sets.
- **Rollback.** `ProfileStore::rollback` (driven by `DriftMonitor`) restores the
  previous qualified profile for new sessions; a `SessionLock` is an owned copy
  and does not change mid-session.
- **Workflow wiring.** In `workflow::attempt` an `Auto` stage consults its provider
  only when its gate attests the live key (`gate_attests_key`); a bare `Passed`
  string does not unlock it. Route reports gain `mode` and `rules_reason`.
- **Calibration.** `calibrate_min_confidence` derives a profile threshold from
  calibration samples or yields none.

Diagnostics added: HPJ016 pinned model not admissible, HPJ017 invalid evidence
outcome, HPJ018 pre-dispatch recheck failed.

Recorded evaluation: `benchmarks/harness/2026-10-04-routing/gate-decision.json`
(12 held-out corpus items; Ollama `qwen2.5:0.5b` backs only the logical
`m-cheap`, other candidates are unavailable cells; the shadow provider is a
fixture table; Laya unavailable, Jev fixture-only). Decision: **no-go, rules stay
active**. Reproduce with `HARNESS_OLLAMA_ENDPOINT=http://127.0.0.1:11434
HARNESS_ROUTING_OUT=<dir> cargo test -p semaprax-harness --test real_tools_v1
real_matched_heldout -- --ignored`.

### Project configuration and CLI wiring (HN-16)

`harness run` reads `[routing]` from `semaprax.harness.toml` and wires it into the one routing
engine: `mode = "rules"|"pin"|"experimental"|"auto"` (`auto` is `qualified-auto`), `pin = "<logical model>"`
and `allow_remote = true|false`. A `pin` is a *project pin* and wins in every mode (`mode = "pin"` without
a `pin` is `SPX-HPB004`); `allow_remote = false` removes remote plans in every mode, while `allow_remote = true`
approves the remote origins of the task's own (endpoint-policy-checked) catalog for project data (without it
remote destinations stay unapproved, as before). Without `mode` the provider's own mode decides, as before.
Evidence is machine-local: `<harness home>/routing/evidence.json` (`semaprax.harness-routing-evidence.v1`, the
JSON of `EvidenceRecord`; loaded only for `auto`, malformed is an error, absent is "no evidence registry"). The
session lock is taken at the run's first route for the live key and compared on every later route of the run.
`recheck_dispatch` runs on the final serialized prompt of every dispatch (`HPJ018` refuses a pinned model;
another model is excluded and the route repeats). Reports carry `route.explanation` (mode, source, rules reason,
applied evidence key and record digest) and `route.policy` (`allow_remote`, `project_pin`). Fixture-origin
evidence cannot unlock `auto` (the rules reason says so).

## `model-route/v2` (MR-01, MR-02, MR-03, MR-15)

`decision.evaluate` has two contract versions; the host implements both. A
descriptor may declare the kind once per version (v1 and v2 entries). v1
(`model-route/v1`, `{task, features, options}` → `{choice, scores, abstain}`)
is byte-for-byte unchanged and is the only payload a v1-only adapter is ever
sent. The router sends v2 only when the adapter negotiated it
(`DecisionInvoker::decision_versions`, from the handle's negotiated set) and
the model profile names the v2 renderer; otherwise it sends v1 and records the
explanation in `RouteDecision.wire.note`. The task registry gains the
compile-time task `model-route/v2`; a provider still cannot register tasks.

**Features.** `TaskFeaturesV2::project(TaskFeatures, RouteSignals)`:
execution domain, task profile (default: the v1 task family), phase, attempt
index (0..=64), host-classified previous failure, verified/no progress
counters (0..=1000), estimated context, structured-output/tools requirements,
input modalities, confidentiality, latency class and remaining budget.
`RouteSignals::default()` is the explicit unknown projection (phase and
previous failure `unknown`, remaining budget `null`). The workflow derives the
signals in `workflow::route_signals` from the attempt number, the recorded
failure stage/code (`proposal` → `parse_schema` or `tool_transport` for host
transport codes, `preview`/`oracle` → `semantic_law`, `checks`/`acceptance` →
`acceptance`) and the task mode; no model classifies anything.

**Candidates and renderer.** Only screened candidates are sent, as selection
ids `m0..m{n-1}` in admissible (opaque-id) order; opaque ids never appear on
the wire and `PreparedRouteV2::opaque` maps an answer back exactly. Each
candidate carries capabilities, context limit, declared quality tier
(`ModelPlan.descriptor.quality_tier`, default `unknown`), and cost/latency
estimates with their basis (`measured`, `configured` (default) or `unknown`;
an unknown value is `null`, never zero, and never ranks as the cheapest under
rules). Strength rank is not sent. The host renderer
`semaprax.route-render.v2` produces `rendered {renderer, instructions, state,
option_labels, digest}`; `digest` is `sha256:` over the canonical JSON of the
first four members (sorted keys, compact, raw UTF-8). A configured label that
contains the opaque id is replaced by the derived label. Bounds: at most 16
candidates, state at most 4096 bytes, labels at most 64 bytes; anything larger
is refused before dispatch (`HPJ019`, fallback `Unsupported`), never truncated.
`max_wire_bytes` = `clamp(2 × model-visible bytes + 2048, 4096, 65536)`.

**Disclosure.** Default `metadata_only`: no excerpt, path, credential, source
or diagnostic text. A bounded excerpt (`RouteSignals.excerpt`, at most 1024
bytes) is sent only when `RoutePolicy.router_excerpt_max_confidentiality`
admits the task's confidentiality — an independent routing-disclosure rule;
approving remote generation does not admit it. `secret` never discloses and a
credential-looking excerpt is withheld.

**Binding.** v2 digests (`V2Digests`: features, candidates incl. the selection
map, renderer incl. rendered digest, disclosure) enter `Digests.v2`, the choice
digest (task `model-route/v2`), the cache key (`CacheKey.schema` and `.scope`,
which also binds the provider scope) and replay (`replay` re-prepares and
refuses drift, `HPJ007`). `EvidenceKey::live_versioned` gives v2 its own task
and normalization (`model-route/v2/semaprax.route-render.v2`), so v1 and v2
never share cache or qualification; the enablement gate is per task.

**Scores (MR-02).** A v2 result is `ResultV2`: choice/abstain,
`abstention_reason`, `scores` with `score_kind` (`option_distribution`,
`candidate_relative`, `none`), labelled `native_confidence`, `calibration_id`
and `call`. Scores must cover exactly the options, be finite in [0,1], the
choice must be an argmax (1e-3) and an option distribution must sum to 1
within 0.05. A scoreless answer is accepted only from a profile that declares
`scoreless`; the host never synthesizes scores. Native abstention is
authoritative. Thresholds: `ProviderProfile.min_option_mass` applies only to
`option_distribution` scores (v1 scores count as one);
`min_confidence` is the deprecated alias with its old behavior (chosen score of
any kind, missing fails closed). Host env `SEMAPRAX_HARNESS_MIN_OPTION_MASS`
sets the former. Reports state what scores are (`wire.scores_are`: option mass
is never a probability of task success). Qualification: `GateSpec.basis` is
`Outcome` (default; matched outcomes only, the only basis a scoreless or
uncalibrated provider can pass) or `CalibratedConfidence`, which requires an
`EvidenceRecord.calibration` of a scored kind bound to exactly the evaluated
key; unknown calibration never passes.

**Call identity and usage (MR-03).** `DecisionCall::Answered.call` carries the
typed `CallMetadata` (adapter, requested/answering model, checkpoint,
`identity_kind`, rendered digest, wire bytes, usage with basis, billing) from
the result's `call` member; no fact is parsed from diagnostic text, and every
identity string is a bounded identifier that cannot carry prose or a
credential. The host requires `rendered_digest` to equal its prepared digest
and `wire_bytes ≤ max_wire_bytes` (else `InvalidResult`). Router admission
reserves the whole prepared payload (a superset of the model-visible text) and
a protocol-derived closed-choice output reserve; settlement uses
provider-reported input usage for a single priced call and otherwise keeps the
reservation uncertain (no usage, timeout after dispatch, several calls), never
zero; an adapter's `billing: local` claim does not zero a priced router. In
`Auto` (qualified) mode a v2 answer is accepted only when the answering
identity matches the profile (`ProviderProfile::verify_identity`:
`immutable_checkpoint` needs the checkpoint, `mutable_service` the exact
answering model version, `local_declared` the declared model; `unknown` never
verifies) — otherwise fallback `IdentityMismatch`, not cached. v1 answers carry
no identity and keep their v1 behavior. `DecisionRecord.identity` journals the
answering identity so resume never re-infers it.

**Adapter identities (MR-15).** `ProviderProfile::configured(AdapterIdentity,
ModelProfile, InstanceConfig)`: adapter = descriptor `provider.id` +
`adapter.version`; model profile = `{profile_id, model, checkpoint,
identity_kind, score_kind, scoreless, max_options, max_state_bytes,
modalities, renderer}` (strict; an unknown renderer or inconsistent
declaration is `HPJ020`); instance = `{instance_id, endpoint, secret_refs}`
(secret names only). The adapter/profile/instance scope enters cache and
evidence keys; a legacy profile (no model profile configured) keeps its v1
keys. The profile is host configuration: the descriptor declares a
`model_profile` string config field and the project sets
`[capability."decision.evaluate".config] model_profile = '<json>'` (forwarded
to the adapter as `SEMAPRAX_HARNESS_CFG_MODEL_PROFILE`; the host also reads
`SEMAPRAX_HARNESS_MODEL_PROFILE`), optional `instance_id`/`endpoint`.
Capabilities are checked before inference (`admits_request`: option count,
state size, modalities → fallback `Unsupported`, no call). Nothing in routing
branches on a vendor or model name. `workflow::decision_open::open_decision`
starts the selected adapter through resolve/trust/lock/negotiate. Recipe: a
compatible new model is a new validated profile (plus its own qualification);
a new protocol is a new out-of-tree adapter with its own descriptor — see the
`decision_adapter` integration test for a non-SystemOne example adopted from a
temporary directory without editing bundled assets.

Diagnostics added: HPJ019 v2 request refused before dispatch (bounds), HPJ020
malformed model profile or threshold; payload `HPA047` malformed call metadata
or prepared-digest/wire-bound mismatch.
