# Runtime model routing v1 (MR-09, MR-10)

Status: implemented in source; local macOS aarch64 contract-fixture evidence
only (see Evidence). No live inference, hosted, or provider-billing claim.

Audience: runtime host authors and agent application authors.

Owner: `src/model_routing/runtime/` (`semaprax::model_routing::runtime`).
Decision engine: [Decision core v1](DECISION-CORE-V1.md). Durable binding:
`DurablePolicyBinding` in `src/model_budget_policy/durable.rs`.

## What it is

A runnable agent may select a suitable model for each new task, and change
model only between durably accepted turns, without weakening its source
contract, its deployment permissions or durable replay. Selection is among a
finite set of host-approved concrete deployment profiles; it never reorders
or substitutes providers inside an existing binding.

## Host API (MR-09)

| Item | Role |
| --- | --- |
| `ProfileSpec`, `ProfileModel` | One host-approved profile: an AgentDeployment v1 document, optional exact provider order, effective limits, logical model metadata (alias, destination, capabilities, modalities, context, cost/latency estimates, strength rank). |
| `ApprovedProfileSet::approve(definition, specs, RoutePolicy)` | Binds every profile with `bind_agent_deployment` against the same definition source and pre-admits its provider order and limits with `DurablePolicyBinding::admit_profile`. Rejects a wrong semantic definition, capability/tool expansion, widened limits, mismatched provider order and metadata that claims more than the deployment admits. No provider contact. |
| `RuntimeFeatures` | Bounded feature projection: task family, context estimate, structured output/tools, required modalities, confidentiality, latency class, remaining cost/latency, router-call allowance, operator pin. |
| `route_new_invocation(set, features, ctx, provider)` | Host screen (modality, allowlist) plus the shared core's screen (privacy, output/tool, context, cost, latency), then rules (no provider: zero calls) or the attached `DecisionInvoker`. Returns `RoutedProfile` and a `RouteRecord`. |
| `RouteRecord` (`semaprax.runtime-route-record.v1`) | Binds profile-set digest, route-policy digest, feature digest, selected profile id, selected bound-deployment digest, semantic definition digest and decision identity (choice digest or pin digest, decision provider, router calls, fallback explanation). |
| `bind_routed_invocation(set, record, target)` | Derives execution, instance and invocation roots from the recorded deployment (`bind_execution_revision`) and binds `DurablePolicyBinding` with its unchanged exact-order checks, before any adapter. |
| `run_routed_invocation`, `start_routed_task` | Commits the route envelope at generation 0, then runs `run_durable_policy_invocation`; every policy-journal commit is wrapped in the envelope (`semaprax.routed-invocation-checkpoint.v1`). |
| `resume_routed_invocation(set, envelope, generation, target, …)` | Reuses the retained route. It takes no decision provider, so it makes zero router calls; the recorded binding must rebind byte for byte. Metadata or alias drift cannot move the invocation; a revoked deployment refuses (`ProfileNotApproved`) and is never rebound. |

Rules: a router names a profile id only. An operator pin is screened like
any candidate and refuses (`InadmissiblePin`) rather than falling back. An
unavailable, timed-out or invalid learned router produces a `Fallback`
record with an explanation when the policy permits rules fallback; a
not-evaluated provider in `Auto` mode is never consulted. No admissible
profile refuses (`NoAdmissibleProfile`) before any record, root or adapter.
A single-profile set is `Trivial` with zero router calls and binds the same
`DurablePolicyBinding` bytes as the unrouted static path.

## Turn routing and handoffs (MR-10)

`RoutedSession` runs a multi-turn agent as a sequence of routed invocations
journaled as `semaprax.runtime-route-session.v1` through the host's
`CheckpointStore`.

- Re-route point: `RerouteBoundary` exists only before the first turn or
  after the previous turn's durable `settled` entry with an accepted
  `continue` transition. A route never changes inside an invocation; each
  turn is a new properly bound invocation, and the old binding and frozen
  plan are untouched.
- Turn features (`TurnFeatures`): declared role, last verified outcome,
  remaining parent allowance and deadline, next-stage capabilities and
  progress counters, recorded by digest in the handoff.
- Allowlists (`SessionPolicy`): role to profile ids, authorized specialists,
  a bounded no-progress `EscalationRule`, maximum turns and delegation depth,
  and the session confidentiality, which no turn may change.
- Handoff (`semaprax.runtime-handoff.v1`): committed state (at most 16 KiB),
  accepted-output digests and tool-result references, the host's
  instruction/acceptance digests and the destination deployment's granted
  capabilities. The transcript is never copied.
- Delegation: `delegate` checks depth, specialist delegability and capability
  containment, then reserves from the same parent allowance through
  `CumulativeBudgetLedger` before dispatch and journals the reservation;
  `open_child` opens a child bounded by the grant (allowance, depth, single
  profile, router lineage); `settle_child` never refunds or overspends. A
  decision provider already on the lineage is refused as router recursion.
- Failure classes stay distinct: transport failover is the deployment's
  ordered provider policy inside one invocation; reasoning escalation is the
  configured rule at a boundary; delegation is an explicit grant. An
  uncertain dispatch (`DurablePolicyRun::Uncertain`) or an uncertain external
  effect (`TurnVerdict::EffectUncertain`) halts the session for the existing
  reconciliation path and is never replayed with another model; a refused or
  cancelled turn halts without a fallback completion.
- Live stop guards: before any decision-provider dispatch a new turn checks
  cancellation, the session deadline against the live clock, and the parent
  allowance through the same ledger. The router envelope's deadline is the
  smaller of the router policy allowance and the remaining enclosing time
  (`RuntimeFeatures::remaining_latency_ms`, itself clamped to the session
  deadline). After routing the live clock and allowance are checked again on
  every route path, rules fallback included; a refusal there journals a
  `failed` entry that names the router work performed, so a repeated turn
  cannot incur more unrecorded routing.
- Effect boundary: before the host's acceptance callback (which may perform
  tool effects) runs, the session journals `effect_intent` for the turn. An
  intent with no terminal entry is reconciled on the next `run_turn` (live or
  resumed) as an `uncertain` effect with zero model and callback calls; only
  model bytes recovered before any durable intent allow exactly one callback.
  The journal schema id is unchanged; `effect_intent` is an additive entry
  kind that earlier readers do not parse.
- Child deadline: `ChildGrant::deadline` carries the enclosing session's
  absolute deadline into `open_child` (and so into the child's retained
  `opened` entry), nested grants inherit it, and a child never broadens it.
- Resume: `RoutedSession::resume` restores the journal; `replay_turn` returns
  completed boundaries without route, model or effect calls; an in-flight
  turn reuses its recorded route and handoff bytes with zero router calls.

## Agent and tool selection (MR-11)

The runtime half of the finite-choice task `choice-select/v1` (engine and wire
in [Harness decision v1](HARNESS-DECISION-V1.md), section
`choice-select/v1`) lives in `model_routing::runtime::choice`. A decision
adapter recommends one already authorized destination; it cannot create one.

| Item | Role |
| --- | --- |
| `granted_tool_options(runtime_profile)` | `tool` options from an Agent Runtime Profile v1 (`BoundAgentDeployment::runtime_v1_profile()`): exactly the tools in `policy.allowed_tool_ids`, with their effects and required capabilities, typed by `tool_schema_type` of their argument/result schemas (cost unknown, never zero). Also returns the granted capability set for `ChoiceQuestion::granted`. |
| `SpecialistRegistry` (`semaprax.runtime-specialist-registry.v1`) | A configured specialist registry: stable id, approved profile, bounded host description, input/output type, data clearance, effects, required capabilities, delegability. Strict closed parse. `options(&set)` builds the `agent` options (an entry naming an unapproved profile is `InvalidConfig`); `grants()` builds `SessionPolicy::specialists` from the same document. |
| `select_choice` (engine) | Screens before inference, takes the zero/one-option paths without a call, consults an adapter only if it negotiated `decision.evaluate` v3, and returns `ChoiceOutcome::{Selected, Abstained, Refused}`. |
| `authorize_tool_choice(selection, live, runtime_profile, arguments)` | Authorize stage for a tool: recheck against the live inputs (`SPX-HPJ024`), against the live profile's grant and contract, and the arguments against the tool's closed argument schema (`SPX-HPJ026`). Returns `AuthorizedTool` (no public constructor). |
| `authorize_specialist_choice(selection, live, registry, set)` | Authorize stage for an agent: recheck against the live inputs (`SPX-HPJ024`) and the live registry and approved set (`SPX-HPJ026`). Returns `AuthorizedSpecialist` (id, profile, deployment digest), dispatched as `TurnRequest::specialist`. |

Refusals of the authorize stage are `RuntimeRoutingError::Choice { code,
message }`. The application dispatches with an exhaustive `match`: only
`Selected` followed by a successful authorize stage runs anything; `Abstained`
(including `unsupported_adapter` and `no_provider`) and `Refused` are explicit
non-actions, never a default destination. The existing boundaries recheck
once more: Runtime v1 refuses a tool the profile does not allow or arguments
its schema rejects, and a session refuses a specialist it does not authorize.
Development-routing qualification never qualifies runtime choice (separate
task, normalization and execution domain in the evidence key).

Examples: `examples/support-routing-project` (route a support request to one
of two approved agents) and `examples/tool-choice-project` (select one of two
granted read-only tools), both through the same adapter contract.

## Non-claims

No live provider, network or billing evidence; the decision provider in the
tests and examples is a deterministic fixture invoker (MR-11 runtime choice
included). No dynamic source
loading, unbounded delegation, parallel child execution or mid-stream route
change. Child sessions run under the same host; there is no distributed
multi-agent runtime.

## Evidence

Local macOS aarch64, offline fixture adapters and a fixture decision invoker:

- `cargo test --locked -p semaprax --test agent_runtime_v1 -- runtime_reroute routed_agent_example runtime_routing`:
  14 passed (6 `runtime_routing` for MR-09, 6 `runtime_reroute` for MR-10,
  1 `routed_agent_example`, plus a no-op helper).
- `cargo test --locked -p semaprax --test agent_runtime_v1 -- execution_revision`:
  the existing retained-root and durable-binding tests pass unchanged.
- `cargo test --locked -p semaprax --lib model_routing`: 11 passed.
- MR-11/MR-14 (2026-10-05): `cargo test --locked -p semaprax --test agent_runtime_v1 -- choice_examples routing_e2e_lane routed_agent_example`:
  7 passed (4 `choice_examples`, 2 `routing_e2e_lane`, 1 `routed_agent_example`);
  `cargo test --locked -p semaprax --lib model_routing::runtime::choice`: 2 passed.
  Deterministic `FixtureChoiceInvoker` and an out-of-tree test adapter only;
  no provisioned decision adapter answered.
