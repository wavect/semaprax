# Model Call Receipt v1

Status: **LOCAL** bounded design + reference implementation, fixture-backed.

Audience: implementers of issue #180 ("Add model-call receipts, replay,
redaction, and billing reconciliation") and reviewers of the receipt/audit
boundary this document adds around
[Live Invocation Contract v1](LIVE-INVOCATION-CONTRACT-V1.md).

This document assumes the reader already knows Live Invocation Contract v1:
the causal journal's record format and ordering rules
(`src/live_invocation/journal.rs`), and the `model.invoke` effect boundary
(`src/live_invocation/model_invoke.rs`). Everything below is additive to that
contract, not a restatement or a second copy of it.

## What already existed at the audit baseline

`src/live_invocation/` (issues #108/#177) already owns the authoritative
per-turn record of a `model.invoke` attempt:

- A causal journal (`journal::JournalEntry`) recording, per turn, exactly
  `TurnOpened`, `RequestIntent`/`ResponseRecorded`/`ResponseFailed`,
  `ProposalAdmitted`/`ProposalRefused`,
  `AuthorizationConsumed`/`AuthorizationRefused`, zero or more
  `EffectIntent`/`EffectObserved`/`EffectFailed` pairs, and one
  `Transition`/`TerminalOutcome` — with `journal::validate` fail-closed on
  any reorder, omission, or post-terminal entry.
- `journal::receipt_projection`, which that module's own docs already name
  as "the entire mechanism a receipt (owned downstream by #180) uses": a
  pure fold over an already-validated journal producing invocation id, turn
  count, model call/failure counts, effect call count, and terminal case.
- `LiveInvocationId`/`LiveInvocationSeed` (`identity.rs`): a stable identity
  derived once from pre-dispatch bytes (ProgramRoot, deployment policy,
  task, budget, interaction schema digest, approved providers), never from a
  response.
- The `model.invoke` effect boundary itself (`model_invoke.rs`):
  `ModelInvocationRequest`/`ModelInvocationOutcome`/`ModelFailure` (a closed
  failure domain), `ProposalDecoder`/`ProposalOutcome`, `AuthorizationGate`/
  `AuthorizationGrant` (the only mint site for authority), and
  `InvocationBudgetHook` (the seam #113/#179 attach cumulative budget policy
  behind).
- `FixtureModelHandler`/`FixtureProposalDecoder`/`fixture_response`
  (`fixture.rs`): deterministic, offline fixtures, including
  `FixtureModelHandler::must_not_be_called()` — a handler that panics if a
  replay path ever reaches it.

None of this is a receipt schema, a redaction boundary, an independent
replay entry point, or a billing reconciler. `journal::receipt_projection`'s
aggregate counts are deliberately coarse (per-invocation totals, not a
per-call shape with root bindings, model/provider/adapter identity, timing,
cost, or provider-reported usage) — #180 asks for the richer per-call shape,
which is what this module adds.

## Why a new module instead of extending `journal`/`model_invoke` directly

`src/live_invocation/**` was leased to another workstream for the session
this module was built in and is read-only from this module's perspective —
its docs, however, already name `#180` as the intended owner of exactly this
layer, and design it to be built *on top of* the journal rather than beside
it. Concretely, a real integration constructs one
[`ModelCallReceipt`](../src/model_call_receipt/receipt.rs) per turn by
folding the same already-`journal::validate`-d entries
`journal::receipt_projection` already folds (turn, `RequestIntent`'s
`request_digest`/`reserved_budget`, `ResponseRecorded`'s `response_digest` /
`ResponseFailed`'s `failure`, `ProposalAdmitted`'s `proposal_digest` /
`ProposalRefused`'s reason) together with the root/attempt/adapter metadata
the deployment already carries (`LiveInvocationId`, ProgramRoot,
DeploymentRoot, InstanceRoot, Agent id, attempt ordinal, model/provider
class, adapter identity). This module never re-implements
`journal::validate`'s ordering rules and never accepts a raw
`model_invoke::ModelHandler` anywhere in its public surface, so it
structurally cannot redispatch a call itself — see "Replay" below.

## Schema: `ModelCallReceipt v1`

`src/model_call_receipt/receipt.rs`. Canonical schema tag
`semaprax.model-call-receipt.v1`. Fields, grouped:

- **Root associations**: `agent_id`, `program_root`, `deployment_root`,
  `instance_root`, `invocation_id`, `turn`, `attempt` (1-based ordinal within
  `(invocation_id, turn)`).
- **Call shape**: `model_class`, `provider_class`, `adapter_identity`,
  `proposal_grammar_digest`, `deployment_policy_digest`.
- **Request commitment**: `task_digest`, `observation_digest`,
  `request_digest`, `request_bytes_len`, `reserved_budget`.
- **Response commitment** (absent until settlement): `response_digest`,
  `response_bytes_len`, `private_payload_reference` (an authenticated
  reference into a separately retained, encrypted payload store — never the
  encrypted bytes or a key).
- **Outcome**: `terminal_stage` (closed vocabulary below), `failure` (the
  closed `ModelFailure::as_str()` tag, if any), `proposal_digest`,
  `proposal_refusal_reason`.
- **Timing**: `reserved_at_ms`, `dispatched_at_ms`, `first_byte_at_ms`,
  `completed_at_ms` — milliseconds since a caller-defined, invocation-scoped
  epoch; this module asserts no wall-clock authority of its own.
- **Local usage/cost**: `local_request_bytes`, `local_response_bytes`,
  `cost_estimate_micros`.
- **Provider evidence**: `provider_call_reference` (a locally generated
  idempotency-key-shaped token this attempt expects a provider invoice to
  cite back verbatim) and `provider_reported` (usage a provider has already
  reported for this exact call, distinct from a later imported invoice row).

A receipt never carries raw prompt bytes, raw response bytes, a credential,
or an authorization header — only domain-separated digests
(`commit_task_bytes`/`commit_observation_bytes`/`commit_response_bytes`/
`commit_proposal_bytes`, each with its own domain separator) and plain
counters. That is what makes a bare `ModelCallReceipt` safe to hand to a
reviewer or an audit capsule by default.

### Attempt lifecycle (`ReceiptStage`)

The closed vocabulary #180 asks for: `Reserved`, `IntentPersisted`,
`Dispatched`, `FirstByte`, `Completed`, `Decoded`, `Accepted`, `Rejected`,
`Cancelled`, `Uncertain`, `Reconciled`. `ReceiptStage::is_settled()` is the
subset (`Completed`/`Decoded`/`Accepted`/`Rejected`) billing reconciliation
requires before it will run at all.

### Canonical rendering and digest

`ModelCallReceipt::render()` builds a fixed-field-order, JSON-escaped
canonical string (the same `format!` + `quote_json` hand-rolled convention
`live_invocation::identity`/`journal`/`model_invoke` already use — never a
reserialized `serde_json::Value`, so redaction or round-tripping can never
silently reorder or reinterpret a field). `ModelCallReceipt::digest()` is a
domain-separated SHA-256 over that rendering. A single mutated byte in the
rendering changes the digest
(`receipt::tests::single_byte_mutation_of_the_rendered_receipt_changes_the_digest`).

### Root binding (`verify_root_binding`)

`RootBindingContext` carries the caller's own trusted agent/root/policy/
grammar identities and the previous attempt ordinal. `verify_root_binding`
rejects a receipt whose `agent_id`, `program_root`, `deployment_root`,
`instance_root`, `invocation_id`, `proposal_grammar_digest`,
`deployment_policy_digest`, or `attempt` (out-of-order relative to
`previous_attempt`) disagrees with that trusted context — each as its own
named `BindingError` variant
(`receipt::tests::verify_root_binding_rejects_every_wrong_root_and_policy_field_individually`).
It does not check request/response digests; that is replay's job (below),
which recomputes rather than trusts.

### Low-entropy payload policy (`PayloadPrivacyClaim`)

A bare commitment digest over a short payload (below
`LOW_ENTROPY_BYTE_THRESHOLD` = 32 bytes) can still be recovered by brute
force even though the plaintext was never transmitted — a two-byte prompt's
digest is not meaningfully private. `PayloadPrivacyClaim::classify` returns
`Withheld` only when an authenticated private reference is attached;
otherwise a short payload gets `DigestOnlyLowEntropyCaveat` (never
`Withheld`), and only a long-enough payload with no reference gets the plain
`DigestOnly` claim. `audit_view`'s rendered view always carries this
classification explicitly rather than a bare "redacted"/"safe" label
(`audit_view::tests::low_entropy_payload_never_earns_a_withheld_claim_without_a_private_reference`).

## Redaction: `ModelCallAuditView v1`

`src/model_call_receipt/audit_view.rs`. Canonical schema tag
`semaprax.model-call-audit-view.v1`.

Because `ModelCallReceipt` itself never carries raw payloads, redaction
needs something real to withhold. `ReceiptPrivateExtras` models the raw
material a real deployment might still retain alongside a receipt (for
debugging, provider-support escalation, or human review): `task`,
`observation`, `response`, `private_payload_reference_material`, plus two
free-text fields a careless handler or adapter might attach —
`adapter_diagnostic_hint` and `provider_error_detail` — and
`authorization_header_echo`. These last three are exactly the shape
`model_invoke.rs`'s own docs warn never belongs in the *journal*
("nothing provider-shaped"); modeling them here lets this module prove its
redaction boundary actually stops them from reaching a reviewer, rather than
merely asserting it never records them in the first place.

`RedactionPolicy` is six independent reveal flags, all `false` by default
(`fully_redacted()`). `redact(receipt, extras, policy)` produces a
`ModelCallAuditView` carrying:

- `receipt_digest`, binding the view to one exact receipt (never mutating
  or reconstructing the receipt itself — two views built from the same
  receipt under different policies always carry the same `receipt_digest`;
  `audit_view::tests::redaction_never_changes_the_bound_receipt_digest`).
- `redacted_fields`: one `RedactedField { name, commitment_digest }` per
  withheld field, so a verifier holding the original `extras` can confirm
  what was actually withheld rather than trusting a silent omission.
- The revealed fields' plaintext directly, for whichever fields the policy
  opted in.
- `PayloadPrivacyClaim` for each payload.

`verify_audit_view(view, receipt, extras)` independently recomputes every
redacted field's commitment from `extras` and rejects a forged commitment
(`AuditViewError::CommitmentMismatch`), and independently checks every
revealed field's plaintext against `extras` and rejects a tampered
plaintext (`AuditViewError::RevealedFieldTampered`).

### Per-field redaction proof

`audit_view::tests::redaction_hides_each_of_six_secret_bearing_fields_individually`
mirrors `std.auth.tests.audit_event_safety`'s structure exactly: a distinct
marker string is placed in exactly one of the six secret-bearing fields at a
time against an otherwise-clean baseline, and:

1. a fully-*revealed* view is asserted to contain that marker (the positive
   control — proves the marker really would surface if not redacted, so the
   test cannot pass merely because "decoded" and "raw" happened to already be
   identical);
2. a fully-*redacted* view is asserted **not** to contain that marker
   anywhere in its rendered text (not merely absent from one struct field);
3. the redacted view still carries a verifiable commitment naming that
   field, and verifies against the real `extras`.

A clean baseline (no marker in any of the six fields) is also asserted to
contain none of the six markers, ruling out a redaction that "passes" only
because the marker was never there to begin with.

## Replay: independent, zero-dispatch

`src/model_call_receipt/replay.rs`. `replay_receipt(receipt, retained,
decoder)` has no parameter of type `model_invoke::ModelHandler` anywhere in
its signature — a caller cannot wire a live handler in even by mistake, so
replay structurally cannot dispatch a new provider call. It:

1. Recomputes `commit_task_bytes(retained.task)` and
   `commit_observation_bytes(retained.observation)` and compares against the
   receipt's embedded digests — **never trusts the embedded field**,
   matching AGENTS.md's "digests identify bytes; recomputation must be
   independent."
2. If the receipt claims a settled response, requires retained response
   bytes and recomputes `commit_response_bytes` against
   `receipt.response_digest` — this is the "reject wrong response" case,
   proven directly by
   `replay::tests::replay_rejects_a_wrong_response_by_recomputing_its_digest_independently`
   (a different, still-well-formed response for the same turn; the embedded
   digest field is untouched, only the bytes checked against it differ).
3. If the receipt claims a decode outcome, requires a `ProposalDecoder`,
   rejects schema drift before ever calling `decode`
   (`ReplayError::SchemaDrift`), then calls the real
   `model_invoke::ProposalDecoder::decode` and recomputes
   `commit_proposal_bytes` over an admitted result, comparing against
   `receipt.proposal_digest` — reproducing the decode, never trusting the
   recorded outcome blindly.

### Zero-dispatch proof

`replay::tests::replay_makes_zero_additional_dispatches_against_the_shared_handler`
builds a receipt through one real dispatch against a shared
`FixtureModelHandler` (asserting `handler.calls == 1` immediately after),
then calls `replay_receipt` five times and asserts `handler.calls` is still
exactly `1` — replay never increments the shared dispatch counter, even
though the handler stayed in scope. `replay_receipt`'s signature having no
handler parameter at all is the structural guarantee; the test additionally
proves it holds in practice by keeping a real, counting handler alive
throughout and confirming it after every replay call. A second check
constructs `FixtureModelHandler::must_not_be_called()` and confirms
`.calls == 0`.

## Billing/usage reconciliation

`src/model_call_receipt/reconciliation.rs`. `ProviderInvoiceRow` is
untrusted external evidence — a caller parses this from an actual provider
invoice or usage export; this module performs no adapter-specific parsing
itself and never treats a row as proof of semantic correctness. Because a
real per-provider tokenizer is out of scope here, "usage units" is a byte
proxy (`local_request_bytes + local_response_bytes` vs.
`tokens_in + tokens_out`) — an explicit, declared approximation, not a claim
of real token accounting.

`BillingReconciler::reconcile(receipt, row)` returns one of the closed
`ReconciliationOutcome` variants, each proven by its own test in
`reconciliation.rs`:

| Outcome | Test |
|---|---|
| `Reconciled` (exact match) | `exact_match_reconciles` |
| `ProviderOverReported { local_units, provider_units }` | `provider_over_report_is_detected_with_the_specific_units` |
| `ProviderUnderReported { local_units, provider_units }` | `provider_under_report_is_detected_with_the_specific_units` |
| `DuplicateInvoiceRow { provider_call_id }` | `a_duplicate_invoice_row_is_rejected_on_its_second_submission` |
| `UnknownCall { provider_call_id }` | `a_row_naming_a_different_call_is_unknown` |
| `WrongAccount { expected, found }` | `a_row_for_the_wrong_account_is_rejected` |
| `Uncertain { reason }` (unsettled receipt, or no row yet) | `reconciliation_is_uncertain_before_the_receipt_settles_or_before_a_row_arrives` |

A discrepancy is always reported with the specific figures or identifiers
that disagreed, never as a bare boolean mismatch, and reconciliation never
mutates the receipt or widens any budget/authority.

## A receipt is evidence, not authority

Nothing in this module imports or references
`model_invoke::AuthorizationGate`; no function here mints, decodes, or
reconstructs an `AuthorizationGrant`. `ModelCallReceipt` carries a
`compile_fail` doc test proving a receipt cannot be passed where a grant is
required — a compile-time type error, not a runtime check:

```rust
fn wants_grant(_: semaprax::live_invocation::AuthorizationGrant) {}

fn feed(receipt: semaprax::model_call_receipt::ModelCallReceipt) {
    wants_grant(receipt); // does not type-check: wrong type entirely
}
```

Run via `cargo test --locked -p semaprax --doc model_call_receipt`.

## No live network call, no real provider, no key

Every test in this module is built from offline fixture bytes and the
deterministic fixtures `live_invocation::fixture` already ships
(`FixtureModelHandler`, `FixtureProposalDecoder`, `fixture_response`).
Wiring a real provider adapter, a real compiled proposal grammar, or a real
invoice-import transport is downstream, human-gated integration work against
the traits this module and `live_invocation` already fix.

## Focused gate

```sh
cargo test --locked -p semaprax --lib model_call_receipt
cargo test --locked -p semaprax --doc model_call_receipt
```

## Explicitly out of scope (unchanged from the issue)

- Storing raw API keys or authorization headers (this module only ever
  models `authorization_header_echo` as a hypothetical leak vector to prove
  redaction stops it — it never itself attaches one).
- Making prompts public by default (a bare receipt carries commitments only;
  `ReceiptPrivateExtras` is a caller-owned sidecar this module never
  constructs on its own).
- Treating provider invoices as proof of semantic correctness.
- Allowing receipt replay to call the provider (structurally impossible —
  see "Replay" above).
- Claiming exact remote execution internals the provider does not attest.
- Real per-provider token accounting (billing reconciliation's "units" are
  an explicit byte-count proxy).
