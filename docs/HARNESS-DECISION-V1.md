# Harness decision layer v1 (HP-10)

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence only.

Audience: toolchain contributors and harness adapter authors.

Owner: `crates/semaprax-harness/src/decision/`; the deterministic engine it
re-exports (screening, rules, policy, plans, validation, rendering, cache and
replay) is the shared decision core in `src/model_routing/engine/`, see
[DECISION-CORE-V1](DECISION-CORE-V1.md) (MR-07). Contract: `decision.evaluate/v1`
in [HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md). Diagnostics letter `J`.

## Current status (2026-10-05)

This section is the one current support statement for model routing and
runtime choice. It supersedes the limitation notes in older sections of this
document, [Harness workflow v1](HARNESS-WORKFLOW-V1.md) and
[Harness bridge v1](HARNESS-BRIDGE-V1.md); those sections are kept as dated
history (HP-04 on 2026-09, HN-16 on 2026-10-04, MR-13 on 2026-10-05).

Three things are kept apart throughout:

- *Supported now*: the code path exists and is exercised by contract tests
  with deterministic fixtures.
- *Live-tested now*: a real model answered through that path on this host.
- *Qualified*: an MR-13 gate passed for exactly that profile, task and
  domain, so `auto` may use it without an explicit selection.

An installed or adopted descriptor proves none of the three. **No learned
decision adapter is live-tested on this host, and the MR-13 gate of record is
not evaluated (no-go): rules stay active in both domains**
(`benchmarks/harness/2026-10-05-routing-matrix/gate-decision.json`).

### Decision providers (choose a route or an option; never generate)

| Profile (`status --routing`) | Adapter | Hosted or local | Tasks (`decision.evaluate`) | Development (harness) | Runtime (application) | Conformance-tested | Live-tested here | Qualified |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `rules` | built-in `semaprax/rules-decision` | in-process | `model-route/v1`, `model-route/v2`; `choice-select/v1` only on its zero/one-option paths | default | default (`recommend_static`, rules route, `select_choice` with no adapter abstains on two or more options) | deterministic tests | not applicable (no model) | default policy, not a learned qualification |
| `jev` | `systemone/jev-hosted` | hosted, vendor endpoint, secret `SEMAPRAX_HARNESS_SECRET_JEV` | v1, v2, v3 | adopt and trust from the checkout; `experimental`, or `auto` after a gate | host `DecisionInvoker` over the host's own approved transport | shared conformance matrix (fake server) | no | no |
| `laya` | `systemone/laya-local` | local, user-selected loopback server | v1, v2, v3 | as above | as above | shared conformance matrix (fake server) | no | no |
| `minijev` | `minijev-local` | local, user-selected loopback worker | v2, v3 (finite-choice letter scoring only, not upstream score/noul parity) | as above | as above | shared conformance matrix (fake engine) | no | no |
| `clef-hosted` | `systemone/cloudflare-clef-hosted`, model `@cf/cloudflare/clef` | hosted (Cloudflare Workers AI), secret `SEMAPRAX_HARNESS_SECRET_CLOUDFLARE` | v1, v2, v3 | as above | as above | contract tests over a fake runner, not the shared conformance matrix | no | no |
| `clef-flash-hosted` | same adapter, model `@cf/cloudflare/clef-flash` | hosted, same secret | v1, v2, v3 | as above | as above | contract tests over a fake runner, not the shared conformance matrix | no | no |
| `clef-local` | `clef-local` | local worker; **unavailable unless provisioned** (weights and worker are installed by the operator, never by setup) | v1, v2, v3 | only after provisioning, adoption and trust | as above | shared conformance matrix (fake model worker) | no | no |
| SDK starter | `packages/semaprax-harness-adapters/examples/decision-adapter-starter` | local keyword scorer, no model | v1, v2, v3 | out-of-tree copy adopts normally | not applicable | shared conformance matrix | not applicable | no |
| fixtures | `FixtureChoiceInvoker`, `decision_fixtures`, test routers | in-process | v1, v2, v3 | tests only | tests and examples only | they are the contract | never evidence | never |

"As above" means the same path as the `jev` row. The runtime consumers
(`model_routing::runtime` routing and `choice-select/v1`) take any
`DecisionInvoker`; the runtime ships no transport to these adapters, so an
application attaches one over its own approved transport and credentials.
Development qualification never qualifies runtime routing or runtime
choice: evidence keys carry the execution domain and the task.

### Generation models (produce the answer; chosen by a route)

| Consumer | Where the model is named | What "actual model" means | Live-tested here |
| --- | --- | --- | --- |
| Development harness | task catalog logical models, served by an adopted `model.generate` adapter | `route.explain.generation_model`: requested, and the answering model the generation adapter reported (or `reported: false`) | HN-16 recorded one local Ollama `qwen2.5:0.5b` generation profile on 2026-10-04 (see *Evidence, modes and qualification*); no other generation profile |
| Runtime application | the `models` rows of each approved AgentDeployment profile | the bound deployment's provider/model (`RouteRecord.deployment`, `model_selections()`) | no; examples and tests use offline fixture adapters |
| External host (Claude Code bridge) | the host itself | Semaprax did not control the parent model; the handshake says so (`parent_model`) | the host's own model, not routed by Semaprax |

### Setup, check and explanation

- Review profiles: `semaprax-harness status --routing [--json]`. Zero
  inference calls. See *Routing setup, check and explain (MR-14)* below for
  the non-billable `--check`, the announced and metered `--probe ... --yes`,
  and the diagnostics `SPX-HPB070..077`.
- Every development route carries `route.explain` (domain, phase, owner,
  pin, candidates and exclusions, decision provider and answering model,
  generation model, score semantics, cache/bypass/fallback reason, evidence
  key, deployment, router overhead). The runtime equivalent is the
  `ChoiceReport` plus the `RouteRecord` of the dispatched deployment.

### Side-by-side examples

| Consumer | Example | Test |
| --- | --- | --- |
| Development: harness phase routing | `examples/harness-phase-routing/semaprax.harness.toml` | `cargo test --locked -p semaprax-harness --test harness_v1 mr14_phase_routing_example` |
| Runtime: deployment-profile routing | `examples/routed-agent-project` | `cargo test --locked -p semaprax --test agent_runtime_v1 routed_agent_example` |
| Runtime: agent selection | `examples/support-routing-project` | `cargo test --locked -p semaprax --test agent_runtime_v1 choice_examples::support` |
| Runtime: tool selection | `examples/tool-choice-project` | `cargo test --locked -p semaprax --test agent_runtime_v1 choice_examples::tool` |
| End-to-end lane (CI) | config, negotiation, selection, authorized dispatch, report, resume | `cargo test --locked -p semaprax --test agent_runtime_v1 routing_e2e_lane` |

Committed configuration names logical models, profile ids and secret
*names* only. Endpoints, credentials and machine paths stay in the harness
home and the environment (development) or in the host's own deployment
configuration (runtime); a developer machine's adoption or trust never
carries into an application deployment.

### Adding a model or an adapter (MR-15)

*A compatible new model or checkpoint is configuration only.* Development:
set `[capability."decision.evaluate".config] model_profile = '<json>'` in
`semaprax.harness.toml` (`{profile_id, model, checkpoint, identity_kind,
score_kind, scoreless, max_options, max_state_bytes, modalities, renderer}`,
strict, see *Adapter identities* below), run `status --routing --check
<profile>`, and qualify it separately: its evidence key differs, so no earlier
evidence transfers. Runtime: a new decision model is a new `ProviderProfile`
for the same `DecisionInvoker`; a new generation model is a new deployment
document plus one `ProfileSpec` in the host routing configuration (see
`examples/routed-agent-project/routing.json`), validated by
`ApprovedProfileSet::approve`.

*A new protocol or inference mechanism is an out-of-tree adapter.* Copy
`packages/semaprax-harness-adapters/examples/decision-adapter-starter`
(descriptor, `adapter.py`, `test_adapter.py`, `conformance_target.py`), give
it its own `provider.id`, replace `score_options`, keep the validation and
result shape, then `adopt` and `trust` it from its own directory and run
`conformance`. For the runtime, implement `DecisionInvoker`
(`evaluate`, `decision_versions`) in the host. No routing, workflow or
runtime dispatch code changes: `routing_setup_mr14::mr14_out_of_tree_starter_copy_decides_through_the_normal_harness_path`
(harness) and `routing_e2e_lane::an_out_of_tree_adapter_serves_both_runtime_consumers_unchanged`
(runtime) prove it with fixture answers. That is extensibility evidence, not
routing quality.

### Known gaps (2026-10-05)

- `harness run` does not forward a provider's declared
  `SEMAPRAX_HARNESS_SECRET_*` variables to the adapter process (the bench and
  the endpoint catalog do), so hosted Jev and Clef can show a secret as set in
  `status --routing` and still not receive it during a run.
- The `minijev-local` descriptor declares no secret although its adapter
  reads `SEMAPRAX_HARNESS_SECRET_MINIJEV`.
- MR-11 runtime choice has no live decision adapter evidence; its examples run
  on the deterministic fixture only.

## Flow

1. Resolve the task in the compile-time registry. Active: `model-route/v1`,
   `model-route/v2` and the runtime choice task `choice-select/v1` (MR-11,
   below; route parsers and replay take model-route tasks only, `HPJ025`).
   Reserved and refused: `tool-select/v1` (superseded by `choice-select/v1`
   with kind `tool`), `context-plan/v1` (`HPJ002`); anything else `HPJ001`. A
   provider cannot add or activate a task.
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

## Matched routing evidence (MR-13)

`bench routing-matrix` (`crates/semaprax-harness/src/bench/routing_matrix/`) collects matched,
per-domain routing evidence and feeds it to the HN-16 registry and gate; it adds no second
registry, engine or gate.

- **Discovery.** Arms come from the approved registry (`semaprax.harness-routing-registry.v1`):
  `rules`, `cost-aware` (TC-10 `choose_start`), one `fixed:<model>` per approved generation
  profile in the task catalog, and one `learned:<provider>/<profile>` per approved decision
  profile (adopted descriptor + MR-15 model profile + instance). A descriptor without
  `decision.evaluate` or a profile without text routing state is listed under
  `excluded_decision_entries`. No vendor or model name appears in code; a new provider is a
  registry entry (the `routing_matrix` test adopts an out-of-tree router that way).
- **Availability.** Requirements are variable names (descriptor `permissions.secrets` plus
  declared endpoints), hardware tags (`SEMAPRAX_MATRIX_HARDWARE`) and a bound live adapter
  session. An unmet requirement makes every cell of the arm `unavailable` with its reason;
  such an arm is `not-evaluated`, never a zero-cost success.
- **Execution and labels.** Each (item, model) runs once and is shared by every arm that chose
  it. Labels come only from the item's independent verifier, `verified_by =
  <kind>:<id>@<revision>`: `compiler`, `tests`, `acceptance` for development;
  `typed_outcome`, `policy_invariant` for application. A router, another model or a fixture is
  never a verifier. Executors declare their class: the fixture table can never yield `real`; a
  row that claims more is downgraded and counted as forged, which blocks qualification.
- **Cost.** The runner prices every attempt from the task set's price book (failures and
  retries included). Router overhead follows MR-03: provider-reported usage of one priced call
  settles exactly; otherwise each call keeps the registry's reserved ceiling, whatever the
  adapter's `billing` claim. Receipts must reconcile: cache read ≤ input, the final attempt's
  verdict equals completion, gateway-owned transport retries are not retried by the host and
  the host-owned path reports no gateway retries. Total cost per accepted task is `null` when
  any executed cell's cost is unknown.
- **Sealing and calibration.** Eval items are sealed (digest of ids, domain and content) before
  anything runs. Calibration items run first and are the only data the cost-aware arm and the
  score calibration read. Eval items whose id is in a profile's `trained_on`, or whose content
  digest equals a calibration or trained item's, count as trained-on and fail the gate.
- **Domains and identity.** `EvidenceKey::bound` binds the execution domain, the candidate-set
  revision and the renderer revision into the key's distribution; `qualify::evaluate_domain`
  adds to the HN-16 `evaluate`: the spec's domain must equal the evidence domain (development
  and application never cross-qualify), the key must equal the live key, every counted outcome
  needs a verifier of that domain, and forged, unreconciled or shadow-only evidence fails.
- **Strata and shadow.** Items carry a stratum (mechanical, tests, localized debug, hard
  semantic, runtime classification, multi-turn recovery, agent/tool selection). Where policy
  prohibits automatic routing (hard or rules-only families) the learned provider runs in
  shadow: its recommendation is executed as a counterfactual and reported, never routed and
  never in the gate record.
- **Gates.** `DomainGateSpec` (`semaprax.harness-routing-gate-spec.v1`) is a reviewed,
  versioned per-domain spec; `check_floor` refuses any threshold weaker than `GateSpec::default()`
  and the run refuses to start with one. The recorded specs (`mr13-development-v1`,
  `mr13-application-v1`) equal the floor and were written before any MR-13 cell ran.
- **Activation and rollback.** Only a `go` registers the record and installs a `SessionLock`
  for exactly that key in the domain's `ProfileStore`; `session_admits` admits a new session
  only for that key. `DriftMonitor::enforce` rolls new sessions back; a running session's lock
  does not change. A no-go keeps rules and claims no saving.
- **Report.** `run-manifest.json` (`semaprax.harness-routing-matrix.v1`) records pins (registry,
  tasks, seal, gate-spec digests, candidate/renderer revisions, executor identity and class,
  learned profile digests), budgets and the real-run cost ceiling, hardware/backend text, the
  real/fixture/unavailable matrix per domain, arm and stratum, verifier identities, billable
  usage (real apart from fixture), cold/warm latency, fallback and escalation rates, per-partition
  metrics, calibration (raw option calibration and downstream success reported separately;
  option mass is not a success probability), gate decisions and activation.
  `gate-decision.json` is its compact summary.

Recorded run: `benchmarks/harness/2026-10-05-routing-matrix/` — fixture lane only (no network,
adapter process or paid call). Rules, fixed and cost-aware arms ran over 71 fixture items in
both domains; every learned profile (the bundled local and hosted decision adapters) is
`unavailable` with its reason. Decision: **not evaluated; rules stay active in both domains**.
The `routing_matrix` integration test regenerates it byte for byte (`SEMAPRAX_MR13_RECORD=1`
rewrites it). The real lane is `run-real.sh` in that directory: it prints the cost ceiling,
then refuses without an operator executor (`SEMAPRAX_MATRIX_EXECUTOR`, protocol
`semaprax.harness-routing-cell.v1`), at least two available generation profiles and
`SEMAPRAX_MATRIX_MAX_USD` at or above the ceiling.

## `choice-select/v1`: runtime tool and agent selection (MR-11)

A finite-choice runtime task over the same decision adapters. One task covers
both destinations through a closed `destination_kind`: `tool` (one of the
caller's already granted tools) and `agent` (one of a configured specialist
registry). It is carried only by `decision.evaluate` **v3**; v1/v2 never carry
it and v3 carries nothing else (`SPX-HPA023`). Engine:
`src/model_routing/engine/{choice,choice_select,choice_fixture}.rs`, re-exported
by `semaprax_harness::decision` and `semaprax_decision_core`.

**Inputs (host data only).** `ChoiceInputs { question, options, policy,
excerpt }`. `ChoiceQuestion` holds the host question schema id, kind,
input/output type ids, confidentiality, remaining budget, allowed effects,
granted capabilities and `allow_remote`. Each `ChoiceOption` is a caller stable
id (`[a-z0-9._-]` segments joined by `/`, 1..=64 bytes: no whitespace, `:`,
quotes or metacharacters, so never a command string, URL or absolute path), a
host/source description (printable ASCII, 1..=96 bytes, no `://`, no
credential), input/output type ids, destination (local/remote), data clearance,
declared cost (unknown is `None`, never zero), effect ids and required
capability ids. The caller supplies the candidates (deployment tool grants or
a specialist registry); a provider can add none.

**Screen before inference.** Every option is checked in order: well-formed,
kind, input type, output type, privacy (remote not allowed, secret data to a
remote destination, data above clearance), budget (cost above the known
remaining budget, or unknown cost against a known budget), effects within the
allowed set, required capabilities granted. Rejected options are reported
(`ChoiceReport.rejected`) and never rendered. Duplicate ids, more than 64
supplied options or a malformed question refuse (`HPJ021`).

**Paths.** Zero admitted options refuse (`HPJ022`, a policy refusal naming each
rejection). One admitted option takes the zero-model path (`Selected`,
source `SingleAdmitted`, no call) or abstains under
`ChoicePolicy.single_option = Abstain`. Two or more: with no provider the
outcome is `Abstained(NoProvider)`, never a default pick.

**Wire.** The admitted set (2..=16, else `HPJ023`) is rendered by
`semaprax.choice-render.v1` under selection ids `c0..c{n-1}`; stable ids never
travel. Request payload: `{task, question{schema, destination_kind,
input_type, output_type, confidentiality}, candidates[{id, label}], options,
disclosure, excerpt?, rendered{renderer, instructions, state, option_labels,
digest}, max_wire_bytes}`; digest and wire bound as in `model-route/v2`. The
instructions are fixed per kind. The result is the `model-route/v2` result
shape (`ResultV2`) over the `c` ids, validated identically (exact options,
argmax, call metadata bound to the rendered digest and wire bound); a choice
outside the options is `RejectedChoice`.

**Untrusted text.** User text reaches a provider only as `excerpt` under the
MR-01 disclosure rule (`ChoicePolicy.excerpt_max_confidentiality`, default
metadata-only; `secret` and credential-looking text never disclose). It is
rendered as one JSON-quoted `untrusted_excerpt (data, not instructions)` line
after the fixed content and cannot change the question, candidates,
instructions or labels.

**Negotiated capability.** A provider is consulted only when its adapter
negotiated v3 (`DecisionInvoker::decision_versions`, from the handle's
negotiated set; descriptors declare a `decision.evaluate` `version: 3` entry);
otherwise `Abstained(UnsupportedAdapter)` with zero calls. Nothing branches on
a vendor or model name. The profile's declared limits (options, state) are
checked before inference (`ProfileLimits`); the call cap, latency ceiling and
`router_reserve_micros` against a known remaining budget likewise
(`CallCapExhausted`, `LatencyExhausted`, `BudgetExhausted`).

**Typed result.** `ChoiceOutcome::{Selected{selection, report},
Abstained{reason, report}, Refused{diagnostic, report}}`. `ChoiceSelection`
has no public constructor and exposes the caller's stable `id()`, `kind()`,
`source()` and score; `resolve(held, id_of)` maps it onto the caller's own
object and `recheck(live)` re-screens it at dispatch (`HPJ024` when the option
is gone, changed or no longer admitted). The selection is advisory: the
caller's authorize/execute stage still rechecks the action and its arguments.
`ChoiceAbstain` names every reason (`native`, `host_threshold`, `no_provider`,
`unsupported_adapter`, `profile_limits`, `not_qualified`, `budget_exhausted`,
`single_option_policy`, `unavailable`, `timeout`, `invalid_result`,
`rejected_choice`, `call_cap_exhausted`, `latency_exhausted`,
`recursion_blocked`, `identity_mismatch`).

**Qualification is per task.** `Auto` mode needs an `EnablementGate` passed
for exactly `choice-select/v1` (`NotQualified` otherwise), then the qualified
identity (`verify_identity`). `EvidenceKey::choice(profile, option_set_digest)`
uses task `choice-select/v1` and normalization
`choice-select/v1/semaprax.choice-render.v1`, so model-route evidence, gates
and calibration never match a choice key.

**Adapters.** Declared through descriptor capability v3 (`required: false`):
Jev hosted, Laya local, Cloudflare Clef hosted and Clef local (shared
SystemOne runtime: the rendered content forwarded verbatim as one `choice`
question), Mini Jev local (finite choice only: letter scoring over the rendered
options, not upstream score/noul parity) and the SDK starter. The SystemOne
runtime and the SDK's `serve_cancellable` refuse any capability version a
session did not negotiate before a handler runs (`SPX-HPK004`). Deterministic
fixtures: `FixtureChoiceInvoker` (Rust; word overlap between the disclosed
excerpt and the option descriptions, abstaining on none or a tie) and
`decision_fixtures.choice_payload/choice_request/validate_choice_result`
(Python). Fixture answers are contract fixtures, not live inference.

Diagnostics added: HPJ021 malformed choice question or candidate set, 022 no
admissible destination, 023 choice request bounds, 024 dispatch recheck
failed, 025 not a model-route task, 026 runtime authorize stage refused the
selection (the live deployment no longer grants the tool or registry entry,
its contract changed, or the arguments fail the tool's closed schema;
`model_routing::runtime::choice`).

## Routing setup, check and explain (MR-14)

Owner: `crates/semaprax-harness/src/profile/routing_status.rs` (setup/status
view, check, probe) and `src/workflow/route_explain.rs` (route report).

**Review the decision-provider profiles** on the existing status verb:
`semaprax-harness status --routing [--json] [--project <dir>]`. It lists
`rules`, `jev`, `laya`, `minijev`, `clef-hosted` (`@cf/cloudflare/clef`),
`clef-flash-hosted` (`@cf/cloudflare/clef-flash`) and `clef-local` (shown as
"unavailable unless provisioned" until a local worker is adopted, trusted and
reachable). Each row shows the tasks its bundled descriptor declares
(`decision.evaluate` v1 → `model-route/v1`, v2 → `model-route/v2`, v3 →
`choice-select/v1`), the selected model, who owns the endpoint
(vendor-hosted, user-selected loopback, none), the declared secret names and
whether each is set (never its value), adoption, trust, readiness and the
qualification state. It makes zero inference calls. Qualification is
`not-evaluated` on this machine unless a gate passed; the MR-13 gate of record
(`benchmarks/harness/2026-10-05-routing-matrix/gate-decision.json`) is
not-evaluated, so rules stay active.

**Non-billable check**: `status --routing --check <profile> [--task <task>]
[--json]` exits 1 with actionable findings and makes zero inference calls (at
most one TCP connect to a configured loopback endpoint). **Explicit probe**:
`status --routing --probe <profile> --yes [--project <dir>] [--python <exe>]`
first prints that it sends one decision request and may incur a billable
provider call; without `--yes` it stops there (`SPX-HPB075`). With `--yes` it
sends one `model-route/v2` request over a synthetic two-candidate catalog (no
task text, no project content) through the project's selected provider and
appends a metered record (router calls, latency, answering model, usage,
billing) to `<harness home>/routing/probes.jsonl`.

| Code | Meaning and fix |
| --- | --- |
| `SPX-HPB070` | A declared secret variable is not set: export it where the harness runs. |
| `SPX-HPB071` | Runtime/worker unavailable: loopback endpoint unset, not loopback, or not accepting connections. |
| `SPX-HPB072` | The adapter does not declare the `decision.evaluate` version the task needs (or unknown task). |
| `SPX-HPB073` | Stale evidence: records exist for another checkpoint; they qualify nothing until re-evaluated. |
| `SPX-HPB074` | The probe was not answered by the provider (abstention, unavailable, invalid); rules decide. |
| `SPX-HPB075` | Probe not confirmed: pass `--yes`. |
| `SPX-HPB076` | Profile not adopted: adopt and trust it (`setup` for bundled adapters). |
| `SPX-HPB077` | Unknown routing profile id. |
| `SPX-HPJ018` | Final-context mismatch at dispatch: the message names the final token count and the fix. |

**Service upstreams.** An `upstream` with no identity probe whose adapter
requests no `process` permission but does request `network` is a network
service or worker (hosted API or user-selected loopback server), not an
executable: `adopt` records a note and adopts no executable, `adopt
--upstream` is refused (`SPX-HPB021`), and `trust` needs no upstream digest.
This covers `laya-local`, `jev-hosted`, `cloudflare-clef-hosted`,
`minijev-local` and `clef-local`, which therefore adopt and trust directly
from the checkout. An upstream with an identity probe, or an adapter that
requests `process` (Graft, RTK, Graphify, Caveman), is still an executable
and `trust` refuses it without `adopt --upstream` (`SPX-HPB033`).

**Route explain.** Every model route in a workflow report carries
`route.explain` (`semaprax.harness-route-explain.v1`): `execution_domain`
(`development`), `phase` (plan, implement or review), `routing_owner`,
`authoritative_pin` (project or user pin, else null), `mode`, `candidates`
(`admitted`, policy-`excluded` with reasons, pre-dispatch `rerouted`),
`decision` (source, choice, decision provider, checkpoint, status, wire
version, the router's answering model and identity kind), `score_semantics`,
`reason` (cache outcome and reason, rules bypass reason, fallback reason,
abstention), `evidence_key` (applied and live digests), `deployment` (the
generation provider and logical model dispatched to), `generation_model`
(requested, and the answering model the generation provider reported, or
`reported: false`) and `router_overhead` (calls, ms, reserved request tokens,
reported router tokens, billing). Plan and review phase log entries carry
their own `explain`. It holds ids, digests, counts and closed reason words
only: never the task text, the rendered router request, a prompt or a secret.

The committed development example is
`examples/harness-phase-routing/semaprax.harness.toml` (no endpoint,
credential or path), exercised by `mr14_phase_routing_example_*`.
