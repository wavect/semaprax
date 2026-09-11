# Universal Semantic Transaction v2 Workflow

Status: implemented bounded profile; local evidence only (see below). Not yet
wired into the CLI or the persistent Workspace service transport; those routes
are separately gated follow-up work, not claimed here.

Audience: compiler contributors, agent-tool authors, and reviewers of
multi-file semantic change composition.

Universal Semantic Transaction v2 Workflow composes an ordered sequence of
already-closed [Universal Semantic Transaction v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md)
`ReplaceExpression` operations into one multi-file feature change. It adds no
second editing route, transaction kernel, or patch-application path: each step
is validated by the exact, unmodified `SemanticTransactionV2::validate` core.
The only new behavior is sequencing — step N is validated against the exact
`ProjectRevision` step N-1 produced, so a real multi-file feature (for example,
editing one function's body in one source file and a caller's body in another)
ends in one immutable, reviewable `ProjectCandidate` instead of N unrelated
candidates.

The implementation is owned by `src/project/semantic_transaction_v2_workflow.rs`
and exports:

```rust
pub struct SemanticTransactionV2Workflow { /* opaque */ }

pub const SEMANTIC_TRANSACTION_V2_WORKFLOW_SCHEMA: &str =
    "semaprax.semantic-transaction-workflow.v2";
pub const MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS: usize = 8;
pub const MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_BYTES: usize = 96 * 1024 * 1024;
```

`SemanticTransactionV2Workflow::derive(base, transactions)` and `::replay(base,
transaction_bytes, expected_digest, bytes)` are the only entry points. Both are
read-only: neither writes a source file, a lockfile, a managed Workspace
generation, or Git state, and neither method exists on any type carrying commit
or `ACTIVE`-pivot authority. This mirrors
[Universal Semantic Transaction Composition v1](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md),
which has the same relationship to the frozen Universal Semantic Transaction v1
envelope that this module has to v2.

## Sequencing, not a new algebra

`derive` takes the workflow's shared original base revision and an ordered
slice of already-parsed `SemanticTransactionV2` values. It rejects an empty
slice and a slice longer than `MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS`.
Otherwise, for each step in order:

1. The step's transaction is validated against the exact revision the
   previous step produced (the original `base` for step 0). This is a plain
   call to the existing `SemanticTransactionV2::validate`; every existing v2
   precondition applies unchanged, including the stale-workspace-revision
   check against the step's own `expected_workspace_revision`.
2. On success, the resulting candidate's `ProjectRevision` becomes the base
   for the next step.
3. On failure, `derive` returns that step's diagnostics (annotated with the
   failing step's index and target, diagnostic codes unchanged) and produces
   no `SemanticTransactionV2Workflow` value at all.

Because expression identities are revision-scoped, a caller cannot accidentally
carry a stale identity or old-source slice computed against an earlier base
into a later step: the reused v2 core reselects and re-verifies both against
the fresh candidate the previous step actually built, and rejects a mismatch
with the existing `SPX-G527` stale diagnostic rather than silently matching a
different expression that happens to share an identity string.

## What one successful workflow proves

A successful `derive` returns one `SemanticTransactionV2Workflow` wrapping the
*final* step's `ProjectCandidate` and one
[`SemanticWorkspaceStructuralDiff`](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md)
comparing the shared original base to that final candidate — the same
structural-diff core Composition v1 uses for rebase and merge, not a second
diff representation. `to_json()` additionally lists every step's own
transaction digest, target, edited source path, and full `impact`/`result`
values, so the workflow's canonical bytes are independently reviewable without
re-running any step.

Because every step's own preservation guarantees compose transitively, a
source file untouched by every step in the workflow is byte-identical to the
shared original base, and a file touched by exactly one step keeps its bytes
outside that step's authenticated expression span exactly as
[Universal Semantic Transaction v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md#preservation-and-artifacts)
already guarantees for a single transaction. Public declaration IDs are
unaffected: `ReplaceExpression` only ever rewrites a body expression, never a
declaration's `@id`.

`replay` reparses every step's submitted transaction bytes, rederives the
workflow from the same base, and requires the submitted envelope bytes and
digest to match exactly, the same closed-replay shape every other transaction
and composition type in this project uses.

## Nonclaims

This does not admit any operation kind other than v2 `ReplaceExpression`; it
does not widen the frozen one-operation v1 or v2 transaction envelopes; it is
not a general multi-operation transaction algebra, a rebase, or a merge (it has
no second parent — see
[Universal Semantic Transaction Composition v1](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md)
for those); it performs no runtime or Project test execution and claims no
behavioral equivalence; and it grants no source-commit, managed `ACTIVE`, or
Git-publication authority. It does not itself expose a CLI command or a
persistent-service transport method — those are separate, narrower follow-up
gates that would reuse this exact core rather than reimplement it.

## Focused gate

The integration evidence is authored as
`tests/project_candidate/universal_semantic_transaction_v2_workflow.rs`, a
module of the existing Project Candidate harness:

```sh
cargo test --locked -p semaprax --test project_candidate \
  universal_semantic_transaction_v2_workflow --no-fail-fast
```

Its cases cover: an ordered two-file and three-file feature change producing
one final candidate; disk bytes proven byte-identical, by full-tree inventory
comparison, when a middle step is rejected (an invalid middle operation
publishes no prefix and produces no draft); a stale expression identity
carried over from the shared original base into a later step rejected rather
than silently applied to a different expression; empty and over-limit step
counts rejected; and exact canonical-bytes replay, including rejection of a
tampered digest and of noncanonical submitted bytes.
