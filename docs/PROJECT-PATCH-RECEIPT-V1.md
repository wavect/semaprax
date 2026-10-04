# Project Patch Receipt v1

Status: implemented retained-candidate summary and refusal/comparison core.
The V2/V3 durable repair terminal route retains its exact compiler-derived
receipt bound to the completed journal and replays it without dispatch. Workflow
adapters and additional assurance observations remain separate work.

Audience: agents and compiler contributors reviewing semantic edit candidates.

`semaprax.patch-receipt.v1` is a compact compiler-owned projection. It carries
the immutable candidate's base and result Project revisions, a bounded preview
of directly changed declaration IDs, and references to compiler-derived
candidate, declaration-catalog, contract-delta, and ownership-delta evidence.
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

Generation independently derives the selected catalog, contract, and ownership
projections from the retained candidate. It does not execute tests, invoke a
provider, run effects, apply source, or publish an artifact. Check rows retain
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
counts. It describes settled runtime evidence; rendering it never dispatches a
test or effect and grants no test, effect, source, or publication authority.

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

The summary exposes four closed compiler-derived families: `candidate`,
`declaration_catalog`, `contract_delta`, and `ownership_delta`. A page accepts
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
The output is read-only descriptive evidence and does not execute tests or
effects, observe a runtime, or grant source or publication authority.

## Evidence availability and limitations

Evidence references identify a schema, digest, subject binding, availability,
and compiler resolver. The candidate library can rederive the listed retained
projections through the closed paged route; it does not add arbitrary paths,
URLs, storage authority, or evidence download. It does not add runtime effect observations, test
execution observations, assurance payload selection, terminal repair replay,
or CLI/service/MCP routes. Those integrations must preserve this receipt's
canonical bytes and its existing authority boundaries.
