# Project Patch Receipt v1

Status: implemented retained-candidate summary, bounded dependency-impact
evidence, and refusal/comparison core. The V2/V3 durable repair terminal route
retains its compiler-derived receipt bound to the completed journal and
replays it without dispatch. Runtime observations use a separate policy
projection; selected assurance evidence has its own verified candidate route.

Audience: agents and compiler contributors reviewing semantic edit candidates.

`semaprax.patch-receipt.v1` is a compact compiler-owned projection. It carries
the immutable candidate's base and result Project revisions, bounded previews
of directly changed declarations and compiler-derived potential reverse
dependencies, and references to compiler-derived candidate,
declaration-catalog, dependency-impact, contract-delta, and ownership-delta
evidence.
It is canonical UTF-8 JSON with one terminal LF and an 8 KiB summary bound.
The outer `receipt_digest` is SHA-256 over the content's canonical bytes using
the `semaprax.patch-receipt.v1\0` domain.

## Candidate receipt and verification

The one-shot CLI uses the same retained service authority and accepts canonical
transaction and receipt values as command operands; it never accepts receipt,
evidence, or candidate paths:

```text
semaprax patch-receipt <project> render <transaction-json> <candidate-digest>
semaprax patch-receipt <project> verify <transaction-json> <candidate-digest> <receipt-json>
semaprax patch-receipt <project> refusal <transaction-json> <requested-candidate-digest>
semaprax patch-receipt <project> verify-refusal <transaction-json> <requested-candidate-digest> <receipt-json>
semaprax patch-receipt <project> compare <left-transaction-json> <left-candidate-digest> <left-receipt-json> <right-transaction-json> <right-candidate-digest> <right-receipt-json>
semaprax patch-receipt <project> evidence-summary <transaction-json> <candidate-digest>
semaprax patch-receipt <project> evidence-page <transaction-json> <candidate-digest> <evidence-id> <handle> <cursor|->
```

```rust
pub fn ProjectCandidate::patch_receipt(&self, expected_candidate: &str)
    -> Result<String, Vec<Diagnostic>>;
pub fn ProjectCandidate::verify_patch_receipt(
    &self, expected_candidate: &str, bytes: &[u8],
) -> Result<String, Vec<Diagnostic>>;
```

Generation independently derives the selected catalog, bounded reverse
dependency impact, contract, and ownership projections from the retained
candidate. Dependency impact is queried for every directly changed declaration
against each checked base and candidate graph where that declaration exists.
It includes only declaration rows beyond the target itself, deduplicates their
stable IDs for the receipt count, and retains phase-specific rows for retrieval.
The fixed reverse query has depth 16, 128 nodes, and 64 KiB per graph. Its
truncation facts remain visible: `details_complete_within_query: false` means
the count is only the observed bounded inventory. These facts describe existing
graph edges; they do not claim behavioral impact, compatibility, or test
coverage.

Generation does not execute tests, invoke a provider, run effects, apply source,
or publish an artifact. Check rows retain
separate categories and explicitly report candidate tests and additional
assurance as `not_run` when no independently bound observation was selected.

Verification replays the candidate from its retained base and compares exact
canonical receipt bytes. Rehashing caller-modified JSON cannot authenticate an
altered receipt. A successful verification proves only the selected retained
inputs, not a later checkout or publication's freshness.

## Authorized runtime observation policy

The V2/V3 repair receipt carries a separate
`semaprax.patch-receipt-policy.v1` projection when the authorized repair host
has already completed its terminal journal. It leaves the compact patch
receipt's canonical bytes unchanged. Its candidate-test entry is `absent` when
the host has no test capability or no settled observation, `partial` when it
has a bounded live observation, and `partial` with feedback-only coverage on a
terminal replay. The effect entry reports current-invocation model/effect
dispatches separately from the terminal journal's cumulative model/effect
counts. Its terminal sidecar also carries the typed dispatcher's effective
call, per-call argument/result, and aggregate charged-byte limits, plus exact
cumulative, current-invocation, and replayed historical charges. A refusal or
failure is explicit; older terminal sidecars report this byte coverage as
`absent`. It never estimates charges from request digests or claims provider
billing. Rendering it never dispatches a test or effect and grants no test,
effect, source, or publication authority.

## Stale selector refusal

```rust
pub fn ProjectCandidate::patch_receipt_refusal(
    &self, requested_candidate: &str,
) -> Result<String, Vec<Diagnostic>>;
pub fn ProjectCandidate::verify_patch_receipt_refusal(
    &self, requested_candidate: &str, bytes: &[u8],
) -> Result<String, Vec<Diagnostic>>;
```

An authenticated but nonmatching candidate selector produces an explicit
`refused_stale_candidate_selector` receipt. It binds the request identity and
retained base/workspace context, leaves `project_revision` null, records a
failed selector check, and records all later checks as `not_run`. It has no
invented resulting candidate identity. Refusal verification recomputes the
same canonical receipt and remains read-only. The one-shot CLI exposes both
operations through the retained semantic-service authority; it accepts the
canonical request and receipt values, never a candidate or receipt path.

## Comparison

```rust
pub fn ProjectCandidate::compare_patch_receipts(
    &self, expected_candidate: &str, bytes: &[u8],
    other: &ProjectCandidate, other_expected_candidate: &str, other_bytes: &[u8],
) -> Result<String, Vec<Diagnostic>>;
```

The `semaprax.patch-receipt-comparison.v1` result verifies both receipt inputs
first. It compares declaration summaries, check rows, and effect-usage objects
only when the receipts both admit candidates and bind the same base Project,
workspace context, and policy/accounting scope. Otherwise it returns
`not_comparable` with stable reasons. It never calculates a universal best
patch score and grants no merge, execution, source, or publication authority.

### Repair alternatives from one base

The durable repair route can reject an attempted candidate before it retains a
final candidate receipt. Compare the two independently verified **final
candidate** receipts from the same retained base. The runnable regression uses
one V2 repair run that rejects its first proposal and retains the corrected
candidate, then constructs a second admitted replacement from that same base:

```sh
CARGO_BUILD_JOBS=1 cargo test --locked -p semaprax-toolchain source_live_cli::repair::tests::receipt_comparison::repair_route_with_rejected_attempt_compares_final_candidates -- --exact
```

The repair report's `runtime_effect_accounting` is cumulative execution
evidence for the whole repair invocation. It includes the rejected attempt and
the final candidate-producing attempt. It is not part of either candidate
receipt: both receipts have `effect_usage.status: "not_applicable"` and the
same `effect_accounting_scope`. The comparison is therefore a candidate-scope
comparison only. A caller must keep runtime totals outside that comparison; a
receipt with a different policy or accounting scope returns `not_comparable`.

## Retained evidence retrieval

```rust
pub fn ProjectCandidate::patch_receipt_evidence_summary(
    &self, expected_candidate: &str,
) -> Result<String, Vec<Diagnostic>>;
pub fn ProjectCandidate::patch_receipt_evidence_page(
    &self, expected_candidate: &str, evidence_id: &str, expected_handle: &str,
    cursor: Option<&str>, options: ProjectPatchReceiptEvidencePageOptions,
) -> Result<String, Vec<Diagnostic>>;
```

The summary exposes five closed compiler-derived families: `candidate`,
`declaration_catalog`, `dependency_impact`, `contract_delta`, and
`ownership_delta`. A page accepts
only one of those identifiers. It never follows a filesystem path, URL, receipt
JSON pointer, or caller-provided evidence document.

Each call reselects the exact candidate and recomputes the complete selected
family. Its handle binds the candidate digest, family and whole canonical
evidence bytes. Cursors additionally bind that handle and the selected page
shape and options. The one-shot CLI fixes evidence pages at 32 items and 64 KiB; use `-`
for the initial page. It accepts no evidence path, document, or caller-selected
resource limit.
The declaration catalog keeps compiler order and can therefore page
multiple stable IDs across source files without dropping cross-file identities.
The dependency-impact family pages individual root/phase/declaration
observations in deterministic compiler order. A page is complete for its
selected bounded inventory; it never represents a truncated impact query as a
complete dependency inventory.
The output is read-only descriptive evidence and does not execute tests or
effects, observe a runtime, or grant source or publication authority.

## Evidence availability and limitations

Evidence references identify a schema, digest, subject binding, availability,
and compiler resolver. The candidate library can rederive the listed retained
projections through the closed paged route; it does not add arbitrary paths,
URLs, storage authority, or evidence download. Runtime effect observations and durable terminal repair replay use a separate
policy/sidecar projection. Candidate test execution remains unobserved by the
base receipt. The one-shot CLI, semantic service, and MCP routes expose the
closed candidate receipt and evidence operations; assurance selection is a
separate verified candidate projection. These routes preserve their existing
authority boundaries.

### Retention and expiry

The receipt itself retains no candidate, source, impact report, or page. Its
evidence is available only while the selected immutable `ProjectCandidate` and
its checked base revision remain held by the caller or by a separate supported
candidate archive route. Recomputing a page never extends that lifetime.

When that retained subject has expired or is unavailable, the library cannot
resolve its evidence reference and must return an unavailable/refused result;
it must not substitute a current workspace, a similarly named declaration, or
receipt-supplied bytes. A historical receipt can still be displayed as bytes,
but its evidence is no longer independently verifiable through this in-memory
route. Durable archive retention, eviction policy, and terminal repair-journal
recovery are separate versioned facilities; their presence is not implied by a
patch receipt.

## Candidate assurance selection

The compact `semaprax.project-candidate-assurance-selection.v1` record is an
additive candidate-library projection for receipts and adapters that need to
name assurance inputs without embedding the assurance summary. It delegates all
envelope replay, source rebinding, obligation derivation, classification, and
coverage calculation to `candidate_assurance_summary`; it does not accept a
caller verdict or rederive assurance itself.

```rust
pub fn ProjectCandidate::candidate_assurance_selection(
    &self, expected_candidate: &str, inputs: &[CandidateAssuranceInput<'_>],
) -> Result<String, Vec<Diagnostic>>;
pub fn ProjectCandidate::verify_candidate_assurance_selection(
    &self, expected_candidate: &str, inputs: &[CandidateAssuranceInput<'_>], bytes: &[u8],
) -> Result<String, Vec<Diagnostic>>;
```

The selection binds its domain-separated digest to the candidate, base Project,
result Project, and selected envelope bytes. It carries a digest/reference to
the independently replayed summary, source and obligation coverage, and visible
unsupported or unobserved limitations. Missing source envelopes remain
`partial`; they are never reported as passed. Verification regenerates the
selection from the exact candidate and supplied envelopes before byte
comparison, so rehashing altered JSON cannot verify it.

The compact selection does not retain supplied envelopes or grant authority.
An adapter that references it must preserve the candidate binding and use an
existing authorized retention owner when envelope retrieval is required. It
never executes tests/effects, accepts a caller-authored assurance verdict, or
changes the immutable patch receipt until that adapter is separately integrated.
