# Project Assurance Manifest v1

Status: versioned additive contract; implementation and executable completion
evidence are owned by the #214 batch and the completion matrix.
Audience: compiler contributors and project assurance integrators.

Project Assurance Manifest v1 extends [Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md)
to a project. It authenticates one retained Project snapshot and reports
obligations across all declared sources. This read-only evidence grants no
execution, publication, signing, review, or other authority.

## Envelope and subject

The schema is `semaprax.project-assurance-manifest.v1`. Its canonical envelope
has exactly the following top-level shape:

```json
{"payload":{},"payload_digest":"sha256:<64 lowercase hex>","schema":"semaprax.project-assurance-manifest.v1"}
```

`payload_digest` binds the canonical `payload` bytes with SHA-256 over domain
`semaprax.project-assurance-manifest.payload.v1\0`, the byte length as a
little-endian u64, and those exact bytes. The digest checks integrity; it is not a signature or source authority. Both
the payload and envelope use recursively sorted JSON and one trailing newline. Unknown
fields, duplicate keys, non-canonical JSON, digest mismatch, and output that
would exceed the selected byte bound are refused. The producer never
truncates or repairs output.

The payload binds `project_revision`, `workspace_revision`, and
`program_root`. It also contains an ordered source inventory; every declared
source is represented by its path, `source_revision`, and `source_digest`.
Inventory order is ascending source path. A verifier
must bind every inventory entry to the retained snapshot and reject omission,
insertion, substitution, path drift, or digest drift.

The `obligations` array is sorted by unique obligation ID. Each record carries
the source path, obligation identity, assurance classification, and the
evidence needed to replay its derivation. An obligation ID is unique across
the whole project. Identical obligations encountered in the entry, public-API,
and test HIR views are emitted once; `coverage.selected_source_functions`
records each source function's `declaration_id`, `source_path`, and selecting
`hir_views`. This deduplication happens before canonical
ordering and is never delegated to a consumer.

## Derivation coverage

For each source, ordinary functions (including authored class methods)
selected in the admitted entry, public-API,
and test views use one shared checked AST/HIR derivation path. The producer
does not invoke the single-file producer once per view, which could duplicate
or disagree on facts. Generic and synthetic functions are represented by
explicit coverage metadata and their function IDs are listed when they are
unselected. Coverage records say which functions and views were selected;
they do not assert that any function or test executed.

All source inventory entries remain bound even when a source contributes no
selected obligation. `coverage.unselected_source_function_ids` and
`coverage.synthetic_function_ids_without_source_owner` identify functions that
receive no assurance record. Exact replay detects modified or omitted coverage.
Coverage is provenance, not runtime evidence.

## Architecture claims

`ArchitectureClaimSet` is evaluated against the exact retained
`ProjectRevision` used to produce the manifest. In v1 the admitted operator is
`forbid_reaches <id> <from> <to>`. A claim contributes an `architecture_law`
obligation with class `compiler_proved` only when the held result proves the
forbidden reachability absent. The Rust API also admits
`protocol_order_bound` (issue #297; see
[Architecture Claims v1](ARCHITECTURE-CLAIMS-V1.md#protocol_order_bound-issue-297)):
a held result records an `architecture_law` obligation keyed to the session
protocol's `@id` with locator `architecture:protocol_order_bound:<claim-id>`;
`forbid_reaches` obligations keep their existing locator and detail bytes. A violated or `unevaluable` claim refuses
generation. The existing checker proves absence of a static reachable path;
the producer never promotes a caller assertion or an unevaluable frontier.

The exact canonical claim-result JSON is included in the payload and bound to
the same retained Project revision. It is replayed and checked as evidence;
it is never treated as authority and cannot be replaced by caller-authored
edges or a caller-asserted proof. Claims are bounded to 256 entries, and each
claim ID and target uses the bounds owned by Architecture Claims v1.

## API and freshness protocol

The public options type is bounded and deterministic:

```rust
pub struct ProjectAssuranceOptions { /* public validated limits; private claims */ }

impl ProjectAssuranceOptions {
    pub fn new(max_bytes: usize, max_obligations: usize) -> Result<Self, Diagnostic>;
    pub fn default() -> Self;
    pub fn with_claims(self, claims: ArchitectureClaimSet) -> Self;
}
```

The authenticated generation path acquires the Project snapshot, derives the
canonical workspace revision and ProgramRoot from that retained revision,
derives obligations and claims, then rechecks all held inputs before returning
the envelope. `generate_from_snapshot` performs the same before-and-after
recheck when the caller supplies a retained snapshot. A stale or substituted
source, manifest, Project revision, workspace revision, ProgramRoot, or claim
result fails closed.

`verify_against_revision` regenerates the exact manifest from independently
trusted revision and source inputs, compares canonical bytes, and rejects both
tamper and drift. Verification does not accept the envelope's own paths,
revisions, digests, coverage claims, or architecture result as authority.

The bounds reuse the Assurance defaults: `max_bytes` is at least 2048 and at
most 16 MiB; `max_obligations` is between 1 and 65536 inclusive. Every bound
failure is a refusal. The existing `semaprax.assurance-manifest.v1` envelope
and its bytes remain unchanged.

## CLI

```text
semaprax project-assurance-manifest <manifest> \
  [--max-bytes N] [--max-obligations N] \
  [--forbid-reaches <id> <from> <to>]
```

`--forbid-reaches` may be repeated up to 256 times. CLI output is exactly the
library envelope, including its final newline, and uses the same fail-closed
bounds and freshness checks. No command option changes the authenticated
Project subject or grants an architecture claim authority.

## Non-claims

This profile does not execute source functions or tests, infer that a listed
function ran, merge unrelated Projects, or certify native/Wasm physical
finalizers. It does not change single-file Assurance Manifest v1 semantics,
source files, Git state, or workspace publication behavior. Completion is
established only by the executable gate recorded in the completion matrix.

## Focused executable gates

`cargo test --locked -p semaprax --test workspace project_assurance_manifest`
exercises the authenticated Project producer, provider coverage, held and
refused claims, independent replay and drift/capacity refusal.
`cargo test --locked -p semaprax --test projections assurance_manifest::project_cli`
executes the actual binary for exact library output and claim/option handling.
These are local report gates, not evidence of native/Wasm execution or hosted support.
