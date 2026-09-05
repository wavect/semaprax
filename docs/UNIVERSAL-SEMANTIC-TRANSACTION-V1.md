# Universal Semantic Transaction v1

Status: additive bounded kernel; focused local evidence passed on 2026-09-05.

Audience: compiler contributors, agent-tool authors, and reviewers of semantic
change evidence.

Universal Semantic Transaction v1 is the first authority-free transaction
envelope over [Canonical Semantic Workspace Revision
v1](CANONICAL-SEMANTIC-WORKSPACE-REVISION-V1.md). It does not replace Project
Candidate v1 or publish source. It binds one exact immutable base revision,
validates one typed intention through the existing complete Project candidate
rebuild, and returns deterministic intent, impact, review, result, and evidence
artifacts.

## Closed v1 envelope

The schema is `semaprax.semantic-transaction.v1`. Canonical JSON is compact,
recursively key-sorted, and terminated by one LF. The envelope contains exactly:

- `expected_workspace_revision`, the Canonical Semantic Workspace Revision v1
  composite digest;
- `operations`, exactly one operation in v1;
- `invariants`, exactly the mandatory Project Candidate semantic-change
  requirements;
- `requested_validation`, exactly canonical source round-trip, complete Project
  admission, ownership and cleanup, native and Wasm emission, and canonical
  workspace revision derivation;
- `requested_authority: "none"`; and
- `schema`.

Input is capped at 1 MiB. Derived artifacts are individually capped at 96 MiB.
`SPX-G525` rejects invalid, unsupported, or noncanonical structure;
`SPX-G526` rejects transaction-owned capacity failures; and `SPX-G527` rejects
stale preconditions or exact-replay mismatch. Underlying parser, verifier, and
Project Candidate diagnostics retain their existing meanings.

## RenameDisplayName

The only v1 operation is `rename_display_name`, with `target`,
`expected_old_value`, and `new_value`. The target must be one explicit,
monomorphic, non-`main` function stable identity. The expected old name is read
from the immutable base before candidate creation. A different base composite
revision or old name fails closed.

Project Candidate's `rename_declaration` implementation performs the actual
typed rewrite, caller migration, canonical-source rebuild, ownership and
cleanup replay, and native/Wasm admission. V1 additionally requires every base
and candidate source to be comment-free canonical source. This bounded rule
prevents the reused candidate formatter from silently erasing comments or
normalizing unrelated trivia. Comment-bearing and noncanonical projects are not
admitted by this first slice.

[Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md) projects whether
a retained declaration currently satisfies these structural prerequisites.
Its `available_operations` result calls the same read-only classifier used by
transaction validation. It is not a reservation, approval, authority grant, or
proof that an arbitrary proposed new name will validate; this transaction
still repeats all checks against its exact immutable base.

## Artifacts and replay

The intent is the exact transaction envelope. The impact schema is
`semaprax.semantic-transaction-impact.v1`; it records exact before/after display
names, stable identity preservation, and base/candidate composite revisions.
It is descriptive compiler projection evidence, not behavioral equivalence or
runtime execution.

The review schema is `semaprax.semantic-transaction-review.v1`. The result
schema is `semaprax.semantic-transaction-result.v1`; it embeds the exact Project
Candidate evidence and complete candidate source-review report, records all
requested validation results, and explicitly records that no commit was
performed and no authority was granted. The evidence schema is
`semaprax.semantic-transaction-evidence.v1`; it embeds and digest-binds all four
artifacts.

Intent, impact, review, and result digests are lowercase `sha256:` values over
`domain || u64le(byte_length) || exact_canonical_bytes`, including the terminal
LF. Their domains, in the same order, are:

```text
semaprax.semantic-transaction.intent.digest.v1\0
semaprax.semantic-transaction.impact.digest.v1\0
semaprax.semantic-transaction.review.digest.v1\0
semaprax.semantic-transaction.result.digest.v1\0
```

Evidence has no self-digest and grants no reusable authorization. Its exact
top-level keys are `artifacts,authority,schema`; `artifacts` is ordered
`impact,intent,result,review`, and each child has exact keys `digest,value`.
Result keys are `authority,base,candidate,operation_results,schema,source_review,
transaction_digest,validation`. All object keys are recursively byte-sorted;
arrays retain the declared order.

Replay parses the exact canonical intent, freshly derives the base composite
revision, re-runs candidate validation and all artifacts, and exact-compares
the complete evidence bytes. A self-consistently edited or cross-paired capsule
does not pass.

## Compatibility and nonclaims

The kernel is additive. Existing Project, workspace, Semantic Workspace Image
v1, Semantic Change v1, and Project Candidate v1 bytes and digest algorithms do
not change. Validation reads only retained immutable state and performs no
filesystem write, commit, generation pivot, or publication.

This badge is not yet a universal operation algebra, multi-operation planner,
general semantic completeness claim, behavioral proof, persistent service,
managed-workspace commit route, source-with-comments rewrite, or authority
grant. Those remain future work.

## Focused gate

```sh
CARGO_TARGET_DIR=target/universal-semantic-transaction-v1 \
  cargo test --locked -p semaprax --test project_candidate \
  universal_semantic_transaction --no-fail-fast
```

The gate covers canonical admission, deterministic artifacts, exact replay,
stale base and old-name rejection, reminted-evidence rejection, direct
ProjectCandidate parity, zero filesystem writes, and unchanged retained legacy
Project/workspace/graph/source bytes.

The repository-wide full profile also reached 1,536 passing library tests on
the same rebased source, then stopped on 11 unrelated existing Project, Wasm,
and WIT failures before doctest, rustdoc, release-build, package, and example
stages. The focused transaction, documentation, module-size, all-target check,
and all-target clippy gates passed.
