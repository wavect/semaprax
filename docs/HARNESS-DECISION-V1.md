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
ledger (output reserve 256) and recorded as an incurred `decision` observation.


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
