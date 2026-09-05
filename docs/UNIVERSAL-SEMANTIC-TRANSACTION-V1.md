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

The additive [Universal Semantic Transaction Composition
v1](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md) derives structural diffs,
rebases a `RenameDisplayName` onto an independently admitted revision, and
orders two sibling rename Candidates through the existing Project Candidate
merge. It preserves the existing rename artifact bytes, remains closed to
`ReplaceBlock`, and returns a validated Candidate rather than a multi-operation
v1 transaction.

## Closed v1 envelope

The schema is `semaprax.semantic-transaction.v1`. Canonical JSON is compact,
recursively key-sorted, and terminated by one LF. The envelope contains exactly:

- `expected_workspace_revision`, the Canonical Semantic Workspace Revision v1
  composite digest;
- `operations`, exactly one operation selected from the closed
  `RenameDisplayName | ReplaceBlock | AddContract | AddDeclaration` algebra;
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

The first v1 operation is `rename_display_name`, with `target`,
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

## ReplaceBlock

`replace_block` carries `target`, `expected_old_block`, and `replacement`. The
target has the same explicit, monomorphic, non-`main`, unique-function
restriction as `RenameDisplayName`. `expected_old_block` is the exact canonical
source slice from the opening `{` through the closing `}` of that function's
body. `replacement` is a typed body expression in the existing closed Project
Candidate expression-constructor grammar; the canonical formatter supplies the
function body's outer braces.

The kernel maps this operation to Project Candidate's existing
`replace_function_body` intention. That path performs typed construction,
complete source reparse, Project admission, ownership/cleanup replay, and
native/Wasm admission. After candidate construction, the kernel independently
locates the target body in both immutable revisions and requires byte equality
for the complete prefix and suffix of its source plus exact equality for every
other source. The result therefore changes only the authenticated block span;
it does not accept caller-provided byte offsets or raw replacement source.

An old-block mismatch is stale (`SPX-G527`). A malformed replacement,
ambiguous/missing target, generic target, or `main` target is invalid
(`SPX-G525`). Candidate expression, type, and ownership diagnostics retain
their existing meanings when the typed replacement itself is invalid. Like
the rename operation, this slice retains the comment-free canonical base
requirement.

## AddContract

`add_contract` carries `target`, `expected_old_contract`, `phase`, and
`predicate`. The target has the same explicit, monomorphic, non-`main`, unique
function restriction as the other operations. `expected_old_contract` is an
exact object with `requires` and `ensures` arrays containing the ordered
canonical source text of every existing predicate. The complete object is
derived from the immutable base and must match exactly before candidate
creation. `phase` is exactly `requires` or `ensures`; `predicate` is one typed
expression in the existing closed Project Candidate constructor grammar.

The kernel maps the operation to Project Candidate's existing `add_contract`
intention. Parameters are in scope and `result` is additionally in scope only
for `ensures`. Complete Project rebuilding owns boolean typing, contract purity,
ownership and cleanup, native/Wasm admission, and the existing expression
bounds. The combined old predicate inventory must be below 1,024 entries.

After candidate construction, the kernel independently reselects the target,
requires the old ordered inventory to be an exact prefix with exactly one new
predicate appended to the selected phase, removes the one canonical added
clause from candidate source, and exact-compares the whole source with the
base. Every other source must be byte-identical.

## AddDeclaration

`add_declaration` carries `target`, `expected_old_module`, and `declaration`.
The target is one unique explicit monomorphic function anchor. `main` may be
the anchor because its signature and body remain unchanged; the new declaration
still cannot be named `main`. `expected_old_module` binds the anchor's normalized
source path, a domain-separated digest of its exact canonical source bytes, and
the ordered complete declaration-identity inventory in that module.

`declaration` is the existing closed Project Candidate constructor for one
function, record, or variant. Before candidate creation, every planned owner,
case, and field identity must be fresh across the complete Project. Candidate
rebuilding owns typed construction, effects, contracts, ownership, cleanup,
native/Wasm admission, and replay. The transaction then requires exactly the
planned new identity inventory, preserves the prior declaration order, proves
one source insertion, and exact-compares every unrelated source. It accepts no
source path, source text, import, manifest change, or authority.

[Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md) projects whether
a retained declaration currently satisfies these structural prerequisites.
Its `available_operations` result calls the same read-only classifier used by
transaction validation. It is not a reservation, approval, authority grant, or
proof that an arbitrary proposed new name will validate; this transaction
still repeats all checks against its exact immutable base.

## Artifacts and replay

Validation derives the base and candidate `ProgramRoot` values from the same
canonical revisions used by the transaction and retains both on the in-memory
artifact set. Additive Rust accessors expose them for service and composition
coordination. Existing intent, impact, review, result, evidence, schema, and
digest bytes are unchanged; their workspace fields are selected through each
root's legacy canonical-workspace binding.

The intent is the exact transaction envelope. The impact schema is
`semaprax.semantic-transaction-impact.v1`; it records the operation-specific
exact before/after display name or block, stable identity preservation, and
base/candidate composite revisions.
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

The separate read-only [Universal Semantic Workflow CLI
v1](UNIVERSAL-SEMANTIC-WORKFLOW-CLI-V1.md) exposes this operation as
`change preview rename-display-name`, `change preview add-contract`, and
`change preview add-declaration`. It derives the exact old display name,
ordered contract inventory, or anchor-module snapshot from the same
authenticated Project generation and prints this kernel's exact result or
evidence. That adapter does not add an operation, wrapper schema, commit path,
or authority.

This badge is one real four-variant operation algebra, not a general or
multi-operation planner, general semantic completeness claim, behavioral
proof, persistent service, managed-workspace commit route,
source-with-comments rewrite, nested-expression replacement surface, contract
removal/replacement, or
authority grant. Those remain future work. Universal Semantic Transaction
Composition v1 continues to admit `RenameDisplayName` only and rejects
`ReplaceBlock`, `AddContract`, and `AddDeclaration`; it has not acquired block,
contract, or declaration rebase or merge semantics.

## Focused gate

```sh
CARGO_TARGET_DIR=target/universal-semantic-transaction-v1 \
  cargo test --locked -p semaprax --test project_candidate \
  universal_semantic_transaction --no-fail-fast
```

The local gate passes fifteen cases covering all four operation variants,
canonical admission, deterministic
artifacts, exact replay, stale base/name/block rejection, malformed and
reminted-evidence rejection, stale old-contract and typed-predicate rejection,
direct ProjectCandidate parity, exact preservation outside an authenticated
body span, appended contract clause, or inserted declaration, zero filesystem
writes, and unchanged retained legacy Project/workspace/graph/source bytes.

The repository-wide full profile also reached 1,536 passing library tests on
the same rebased source, then stopped on 11 unrelated existing Project, Wasm,
and WIT failures before doctest, rustdoc, release-build, package, and example
stages. The focused transaction, documentation, module-size, all-target check,
and all-target clippy gates passed.

## Additive exact base selection

`validate_exact` first selects a dual-keyed `ExactProgramContext`, then runs the
unchanged v1 transaction over its retained Project. Artifacts expose the exact
base ProgramRoot v2 in memory only. Candidate ProgramRoot v2 is deliberately
absent: Project Lock v1 verification still requires a held snapshot, so an
in-memory candidate cannot yet freshly replay every external fact. Existing
transaction, result, review, impact, and evidence bytes remain unchanged.
