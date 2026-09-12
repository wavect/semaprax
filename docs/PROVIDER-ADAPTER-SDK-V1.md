# Provider Adapter SDK v1

Status: **LOCAL** bounded design + reference implementation, fixture-backed
only. No live network call, no real provider, and no credential were used to
produce any evidence this document or its implementation cites.

Audience: implementers of issue #181 ("Create a provider adapter SDK and
deterministic model-provider conformance suite") and reviewers of the
adapter/conformance boundary this document adds around
[Live Invocation Contract v1](LIVE-INVOCATION-CONTRACT-V1.md),
[Model Budget Policy v1](MODEL-BUDGET-POLICY-V1.md), and
[Model Call Receipt v1](MODEL-CALL-RECEIPT-V1.md).

This document assumes the reader already knows Live Invocation Contract v1's
`model.invoke` effect boundary (`src/live_invocation/model_invoke.rs`):
`ModelHandler`, `ProposalDecoder`, `AuthorizationGate`, `InvocationBudgetHook`,
and the closed `ModelFailure` failure domain. Everything below is additive to
that contract, never a restatement or a second copy of it.

## What already existed at the audit baseline (2026-09-11, `ae25c6a4`)

- `live_invocation::model_invoke` already defines the provider-independent
  `model.invoke` request/response shape and the injected `ModelHandler`
  trait a real deployment binds to one concrete transport. It is a single
  blocking call: one request in, one `ModelInvocationOutcome` out.
  `live_invocation::fixture` ships only deterministic, scripted
  implementations of every seam.
- `model_budget_policy::classification::AttemptOutcomeClass` and
  `retry_is_permitted` already define the closed, transport-agnostic
  retry-safety vocabulary; `model_budget_policy::provider_policy` already
  owns the exact ordered, confidentiality-checked failover sequence a
  deployment may switch across.
- `model_call_receipt` already owns the canonical, redaction-aware,
  replayable record of one settled attempt, and `agent_interaction_schema`
  (with its `streaming_proposal_decode` extension) already owns the one
  real, compiler-derived Proposal grammar and its incremental, chunk-order-
  agnostic decoder.
- No public, stable, capability-declaring **adapter** interface existed: a
  real provider integration had nowhere to declare what it supports before
  being dispatched against, and no shared corpus existed to check that
  declaration, or a candidate implementation's behavior under hostile
  input, before describing it as conforming.

## What this module adds

`src/provider_adapter_sdk/` is new. It sits *behind*
`live_invocation::model_invoke::ModelHandler`, never replacing it: a real
deployment still binds exactly one `ModelHandler` per deployment, and
adapting one `ProviderAdapter` into that seam is downstream integration
work, out of this module's scope. What this module adds is the richer,
capability-declaring, event-streaming shape a third-party provider
integration implements once, plus the shared suite that checks it.

### The ABI (`adapter.rs`, `capability.rs`)

- `AdapterCapabilities`: the adapter's self-report — identity/version,
  provider profile, supported structured-output modes, whether it streams,
  where its usage/cost numbers come from, its cancellation semantics, which
  `AttemptOutcomeClass` values it promises are safe to retry, its endpoint
  policy, and its byte/token ceilings. Declared once, before any dispatch.
- `AdapterInvocationCapability`: the explicit, non-ambient grant required to
  construct and drive an adapter, mirroring
  `ModelInvokeCapability::grant` exactly.
- `ProviderAdapter`: `capabilities`, `start`, `poll`, `cancel`. `poll`
  returns one `AdapterEvent` (`Delta`/`Usage`/`Completed`), `Pending`, a
  terminal `Settled`, or a terminal `Failed { failure: ModelFailure, .. }` —
  reusing `live_invocation::model_invoke::ModelFailure` as the one closed
  error-normalization vocabulary, not a second one.
- `negotiate(caps, required)`: the *only* function that decides whether a
  declared adapter may be dispatched against. It refuses, unconditionally
  and before any call to `start`: an adapter that declares
  `EndpointPolicy::AdapterDeclaredAmbient` (an ambient endpoint/proxy/
  credential lookup instead of `HostInjected`), or one that declares a
  retry-unsafe `AttemptOutcomeClass` as retryable. It additionally refuses,
  against a caller's stated `RequiredCapabilities`: an unsupported streaming
  or structured-output-mode requirement, or a byte budget larger than the
  adapter's declared maximum.

### The driver (`conformance.rs`)

`drive_to_settlement` is the one place this SDK concatenates `Delta` events
and enforces the rules a conforming adapter must never violate:

| Rule | Violation code |
| --- | --- |
| Exactly one `Completed` per request | `ADAPTER-DUPLICATE-COMPLETION` |
| No event after `cancel()` was called | `ADAPTER-LATE-AFTER-CANCEL` |
| Usage never regresses across snapshots | `ADAPTER-CONTRADICTORY-USAGE` |
| Concatenated response stays within the declared bound | `ADAPTER-OVERSIZED-RESPONSE` |
| No `Delta`/`Usage` after `Completed` | `ADAPTER-EVENT-AFTER-COMPLETION` |
| A terminal outcome is reached within a bounded poll budget | `ADAPTER-POLL-BUDGET-EXCEEDED` |

A mid-stream disconnect (the adapter itself returns `Failed`) is not a
violation of any of these: the driver surfaces it unchanged, matching
`ModelFailure`'s existing closed vocabulary.

### The report (`report.rs`)

`ConformanceReport` binds adapter identity/version, provider profile, the
named test corpus (`TEST_CORPUS_ID`), one canonical rendering of the
adapter's observed capabilities, an ordered list of named case results, and
a fixed set of nonclaims. `render`/`digest` are canonical and deterministic,
mirroring `ModelCallReceipt`'s own "commitments, not authority" discipline: a
fully passing report is local evidence that the suite ran and observed no
violation, never itself a support or publication decision
(`NONCLAIM_NOT_A_SUPPORT_DECISION`).

### The fixtures (`fixture_adapters.rs`, `hostile.rs`)

Two materially different conforming adapters — `ScriptedBatchAdapter`
(single-shot, non-streaming) and `ScriptedStreamingAdapter` (multi-event
streaming) — both pass the same corpus, demonstrating the ABI is not shaped
around either transport style. `RecordedReplayAdapter` replays a fixed
recording verbatim; there is no field in it capable of an outbound call, so
"replay reproduces the recording without dispatch" is a structural property
of the type, not a runtime check. `hostile.rs` ships one adapter (or
capability declaration) per named violation above, plus an ambient-endpoint
declaration, an unsafe-retryable-class declaration, and a credential-holding
adapter used only to prove nothing it holds ever appears in a report.

## Nonclaims

- **No live network, no real provider, no key.** Every adapter this crate
  ships is pure, in-memory, scripted data. A real provider transport is
  downstream integration work maintained outside this crate's core
  semantics, per this issue's own "Explicitly out of scope".
- **Cancellation is a request, not proof.** `CancellationSemantics` has no
  `Guaranteed` variant. `AdapterPoll::Failed { failure: ModelFailure::Cancelled, .. }`
  proves only that this process observed cancellation, never that a
  provider stopped processing or billing.
- **Usage/cost is adapter-reported, not independently verified.** This
  suite checks internal consistency (usage never regresses), never
  agreement with real provider billing.
- **A passing report is not a support decision.** Generating a report, even
  a fully passing one, is not itself a decision to describe an adapter as
  supported — that remains a separate, human, out-of-band decision.
- **Two materially different *fixture* adapters, not two live providers.**
  This module proves the ABI is not provider-shaped using two offline
  transport styles (batch and streaming). A first and second *live*
  provider adapter are downstream integration work this module does not
  ship, matching the human-gated, network-off scope of this session.

## Relationship to read-only modules

`src/live_invocation/`, `src/model_budget_policy/`, `src/model_call_receipt/`,
`src/streaming_proposal_decode*`, and `src/agent_interaction_schema/` are
read-only from this module's perspective; it only imports their existing
public surface (`ModelFailure`, `AttemptOutcomeClass`/`retry_is_permitted`,
`CompiledInteractionSchema`/`compile_agent_interaction_schema`) rather than
duplicating any of it. It defines no new wire format for the existing
Proposal grammar: an adapter's assembled response bytes are still decoded,
end to end, by the exact same `CompiledInteractionSchema::decode` the
whole-document path uses (`provider_adapter_sdk::tests::
an_adapter_response_assembled_from_a_split_multi_byte_character_decodes_via_the_real_compiled_schema`
and its negative control,
`a_field_tampered_after_reassembly_is_refused_by_the_real_compiled_schema_not_by_this_sdk`).
