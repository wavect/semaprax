# Harness decision layer v1 (HP-10)

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
