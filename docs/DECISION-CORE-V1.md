# Decision core v1 (MR-07)

Status: implemented in source; local macOS aarch64 evidence only (see Evidence).

Audience: runtime and toolchain contributors, harness adapter authors.

Owner: `src/model_routing/engine/` (also the workspace crate
`crates/semaprax-decision-core`). Behavior specification:
[HARNESS-DECISION-V1](HARNESS-DECISION-V1.md); wire contract:
`decision.evaluate` v1/v2 in [HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md).

## Placement

One implementation of policy-first model routing serves both the private
development harness and deployed agents. The source lives in the standalone
package at `src/model_routing/engine/` and is compiled twice:

| Mounting | Path | Consumers |
| --- | --- | --- |
| `semaprax::model_routing::engine` | module of the standalone package | the public runtime ([`semaprax::model_routing`](../src/model_routing/mod.rs)) |
| `semaprax_decision_core` | `crates/semaprax-decision-core`, `[lib] path` into the same directory | `semaprax-harness`, `semaprax-toolchain` |

The standalone package cannot take a path dependency on a workspace crate:
`cargo package --locked -p semaprax` verifies the archive against the registry,
the package gate must verify the actual archive, and a path dependency would
make the published compiler depend on an unpublished registry package (the
reason the former `semaprax-oci-package` crate was retired; see
[Architecture](ARCHITECTURE.md)). Nor may it carry `semaprax-harness`
([HARNESS-TOOLCHAIN-V1](HARNESS-TOOLCHAIN-V1.md)). Mounting one source tree in
both places keeps a single implementation with no new root dependency:
`serde_json` and `sha2` are already root dependencies at the same pins.
`semaprax-decision-core` is `publish = false`; there is no publish order to
maintain. Files under `engine/` refer to each other through `super::` only and
never name `crate::`, so both mountings resolve alike.

The core performs no filesystem, process, network, environment or clock
access. Elapsed time, adapter answers and call metadata arrive through the
host's invoker as untrusted data.

## What is shared and what stays in the harness

Shared (`engine/`): route request and feature types (v1 `TaskFeatures`, v2
`TaskFeaturesV2`/`RouteSignals`), candidate plans and descriptors, the hard
screen, rules selection, `RoutePolicy`, `FrozenRoutePlan`/`AttemptLedger`,
advisory-result validation (`wire`: v1/v2 request and result shapes,
`ResultV2`/`CallMetadata`), the `semaprax.route-render.v2` renderer and digests,
choice digests, `DecisionCache`, `DecisionRecord`/`replay`, the provider
profile and enablement gate, the bounded `DecisionRequest`, the MR-15
`DecisionInvoker` trait, `router::decide` and the runtime `boundary`. The
diagnostic type (`Diagnostic`, code plus message) is shared too, so a refusal
is identical on both paths; the harness re-exports it as `HarnessDiagnostic`.

Harness only: the envelope-form invoker trait and its adaptation
(`decision::provider`: `impl semaprax_decision_core::DecisionInvoker for dyn
DecisionInvoker`), `HostDecisionInvoker` over adapter processes, the
`decide` CLI, evidence registry, qualification, governed routing, cost routing
(pricing/usage settlement) and the toolchain policy bridge. Every
`semaprax_harness::decision::*` path is kept through re-exports.

## Runtime boundary

```rust
use semaprax::model_routing::{recommend, recommend_static, RouteInputs, RouteContext};

let rec = recommend_static(&inputs, &ctx)?;            // no provider: rules, zero calls
let rec = recommend(&inputs, &ctx, Some(&mut provider), None)?; // host-attached adapter
let chosen = rec.select(&host_admitted, |d| d.selection_id())?; // host's own object
```

- `RouteInputs` holds the host-admitted candidates (`ModelPlan`s carrying the
  host's own selection ids), the task features, the budget and the
  `RoutePolicy`. The hard screen runs first; the provider sees only screened
  ids (v2: `m0..`, mapped back by the core).
- With no provider attached the result is the rules (or trivial) decision, no
  invoker exists, and nothing can reach a network.
- A host attaches an approved decision adapter by implementing
  `DecisionInvoker` over its own transport; the core sends the bounded
  `DecisionRequest` and validates the answer exactly as the harness does.
- `Recommendation` has no public constructor and holds only candidate ids,
  digests and a frozen order over admitted ids. No type in the core
  represents a deployment, tool grant, credential or transport, so provider
  strings cannot become one: an unknown choice is `RejectedChoice`, extra
  members are `InvalidResult`, both falling back to rules or refusing.
- A recommendation is advisory. The host's existing authority
  (`BoundAgentDeployment`, `DurablePolicyBinding`) rechecks it at dispatch;
  binding recommendations onto pre-bound deployment profiles is out of scope
  here.

## API stability and serialization versioning

- Supported API: the items re-exported by `semaprax::model_routing` and by
  `semaprax_decision_core`'s root. They follow the standalone package's semver;
  until 1.0 a minor release may change them, with a changelog entry. Module
  internals (`shape`, `text`, crate-visible helpers) are not supported API.
- Serialized forms are versioned by name, never changed in place:
  `model-route/v1` and `model-route/v2` payloads, renderer
  `semaprax.route-render.v2`, decision record schema
  `semaprax.decision-record.v1`, and the digest domains
  (`semaprax.decision.choice.v1`, `semaprax.decision.policy-budget.v1`,
  `semaprax.decision.provider-scope.v1`, `semaprax.decision.cache-scope.v2`, and
  the rest named in the source). A change to any byte they cover takes a new
  name and keeps the old reader.
- Digests are canonical JSON (sorted keys, compact, non-ASCII raw) over
  `json::canonical`; the cross-language v2 vector is pinned by
  `render::tests::rendered_digest_matches_the_pinned_cross_language_vector`.
- Diagnostic codes (`SPX-HPA0xx` payload, `SPX-HPJ0xx` decision) are stable
  identifiers; message text is not.

## Evidence

- `cargo test --locked -p semaprax-decision-core`: engine unit tests including
  `boundary::tests` (no provider means rules with zero invoker calls; provider
  strings cannot add a candidate, grant or endpoint; confidentiality holds), and
  `tests/closure.rs` (core depends on `serde_json`/`sha2` only; the standalone
  package's `Cargo.lock` closure has no private, decision-core or model-runtime
  crate and no path dependency; one mounted implementation with no `crate::`
  or ambient I/O).
- `cargo test --locked -p semaprax-harness --lib --test harness_v1`: the
  existing decision, v2, scores, cache/replay, pin, confidentiality, cost and
  fallback tests unchanged, plus `decision_core` (identical decisions,
  rejection reasons and refusals through the harness path and the runtime
  boundary).
