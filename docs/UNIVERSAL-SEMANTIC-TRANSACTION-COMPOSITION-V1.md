# Universal Semantic Transaction Composition v1

Status: additive authority-free composition core; five focused integration
cases passed locally on 2026-09-05.

Audience: compiler contributors, agent-tool authors, and reviewers of
revision-bound semantic change composition.

Universal Semantic Transaction Composition v1 adds a bounded structural diff,
single-transaction rebase, and explicit ordered merge over the existing
[Universal Semantic Transaction v1](UNIVERSAL-SEMANTIC-TRANSACTION-V1.md)
`RenameDisplayName` slice. It reuses complete Project Candidate validation and
conflict selection. It does not create a second change engine, publish source,
or widen the frozen one-operation transaction envelope.

The implementation is owned by
`src/project/semantic_transaction_composition.rs` and exports:

```rust
pub struct SemanticWorkspaceStructuralDiff { /* opaque */ }
pub struct SemanticTransactionRebase { /* opaque */ }
pub struct SemanticTransactionMerge { /* opaque */ }

pub enum SemanticTransactionMergeOrder {
    LeftThenRight,
    RightThenLeft,
}

pub const SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_SCHEMA: &str =
    "semaprax.semantic-workspace-structural-diff.v1";
pub const SEMANTIC_TRANSACTION_REBASE_SCHEMA: &str =
    "semaprax.semantic-transaction-rebase.v1";
pub const SEMANTIC_TRANSACTION_MERGE_SCHEMA: &str =
    "semaprax.semantic-transaction-merge.v1";
pub const MAX_SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SEMANTIC_TRANSACTION_COMPOSITION_BYTES: usize = 64 * 1024 * 1024;
```

`SemanticTransaction::rebase` and `SemanticTransaction::merge` are convenience
entry points to the same core. Every returned object is immutable derived
evidence. No method applies it to disk or to a managed Workspace generation.

## Canonical structural diff

`SemanticWorkspaceStructuralDiff::derive(candidate, expected_candidate)`
requires the exact retained Candidate digest. It independently derives the
Canonical Semantic Workspace Revision for the Candidate's original base and
result and returns:

- complete base and Candidate bindings for the Project revision, composite
  workspace revision, four component digests, and all nine typed-node digests;
- ordered names of the components and nodes whose exact digests changed;
- the existing compact Project Candidate semantic-delta root catalogue, bound
  by a separate digest;
- the existing exact Candidate source-review report, also separately
  digest-bound;
- the exact Candidate digest, report limit, `authority: false`, and bounded
  nonclaims.

The classification is exact canonical-revision and authored-structure
projection equality. It is not behavioral equivalence, complete dynamic
impact, a source patch, trivia preservation, runtime or test execution, or
source-publication authority.

`SemanticWorkspaceStructuralDiff::replay` validates the submitted schema and
canonical bytes, checks the expected Candidate and report digests, freshly
derives the complete report from the retained Candidate, and exact-compares the
result. Submitted JSON is never trusted as HIR or authority.

## Rebase

`SemanticTransactionRebase::derive` accepts one typed Semantic Transaction v1,
its original admitted `ProjectRevision`, an independently admitted destination
revision, and the exact expected destination workspace revision. It:

1. derives and checks both canonical workspace revisions;
2. freshly validates the original transaction against its bound base;
3. reruns the shared `RenameDisplayName` eligibility classifier on the
   destination;
4. delegates conservative conflict selection and full history replay to
   `ProjectCandidate::rebase`;
5. remints the same single rename with the destination's exact current display
   name and workspace revision;
6. freshly validates that rebased transaction; and
7. requires exact source, Project revision, Candidate digest, and Candidate
   evidence parity between the Candidate rebase and reminted transaction.

The result embeds and binds the original and rebased transactions, Project
Candidate reconciliation report, exact source review, structural diff, result
Project/Candidate/workspace identities, and validation facts. The destination
must remain inside the comment-free canonical RenameDisplayName v1 domain. A
missing, ineligible, conflicting, already-satisfied, or stale destination fails
closed rather than guessing a source merge.

`SemanticTransactionRebase::replay` parses the exact original transaction,
freshly performs this complete derivation against both admitted revisions, and
exact-compares the caller's report bytes and digest.

## Ordered merge

`SemanticTransactionMerge::derive` accepts two distinct Semantic Transaction
v1 values over the same exact canonical workspace revision and an explicit
`left_then_right` or `right_then_left` order. Both parents are freshly
validated. Their stable-ID targets must be distinct.

The implementation delegates to `ProjectCandidate::merge`, compensating for
that API's receiver/argument application order so the public order name matches
actual replay order. Candidate owns the common-history, conflict,
canonical-source, full Project admission, ownership/cleanup, native/Wasm
emission, and history-bound checks. The result binds both parent transaction
and Candidate digests, the selected order, Project Candidate reconciliation,
the complete admitted result identities, source review, structural diff, and
validation facts.

The result is a validated `ProjectCandidate`, not a Semantic Transaction v1.
The existing transaction schema continues to admit exactly one operation.
Different orders may have different Candidate history and composition digests
even when their final canonical source bytes match. V1 makes no commutativity,
automatic-order selection, or general semantic-merge claim. A conservative
rejection is not proof of fundamental incompatibility.

`SemanticTransactionMerge::replay` parses both exact parent transactions,
revalidates them against the shared base, repeats the selected Candidate merge,
and exact-compares the complete result bytes and digest.

## Canonical bytes, digests, bounds, and diagnostics

All three reports are compact recursively key-sorted UTF-8 JSON with exactly
one terminal LF. Their lowercase SHA-256 digests hash:

```text
domain || u64le(report_byte_length) || exact_report_bytes
```

using these domains:

```text
semaprax.semantic-workspace-structural-diff.digest.v1\0
semaprax.semantic-transaction-rebase.digest.v1\0
semaprax.semantic-transaction-merge.digest.v1\0
```

The embedded semantic-delta catalogue, source review, and Candidate
reconciliation have separate domain-separated digests. Structural diffs are
capped at 32 MiB; rebase and merge reports are capped at 64 MiB. Existing
transaction, Candidate, semantic-delta, source-review, Project, and canonical
workspace bounds continue to apply before the outer report is returned.

| Code | Meaning |
| --- | --- |
| `SPX-G536` | Invalid digest grammar, malformed or noncanonical composition material, or an invalid embedded projection. |
| `SPX-G537` | Composition input, output, or delegated Candidate capacity exceeded. |
| `SPX-G538` | Stale base, destination, parent, digest, or exact replay result. |
| `SPX-G539` | Conservative composition conflict, including same-target parents or failed Candidate reconciliation. |

Other parser, verifier, transaction, Candidate, Project, or canonical-workspace
diagnostics retain their owning codes when those layers reject first.

## CLI projection

The read-only Universal Semantic Workflow CLI has additive exact-output forms:

```text
semaprax change preview <project> rename-display-name <stable-id> <new-name> [--revision <digest>] --structural-diff
semaprax change rebase <base-project> rename-display-name <stable-id> <new-name> --onto <onto-project> [--revision <digest>] [--onto-revision <digest>]
semaprax change merge <project> rename-display-name <left-id> <left-new-name> --with rename-display-name <right-id> <right-new-name> [--revision <digest>] --order <left-then-right|right-then-left>
```

Preview prints the exact structural-diff JSON. Rebase and merge print the exact
core report selected by their typed inputs. A locally passed four-case Workspace
harness proves exact core parity, closed/stale grammar, unchanged legacy preview
result/evidence bytes, and zero writes across every selected root. Rebase authenticates and finally
rechecks both explicitly selected Projects; merge uses one authenticated
shared-base Project. The CLI does not accept transaction JSON, evidence files,
source paths, output paths, automatic merge order, or a commit flag. These CLI
adapters do not widen the five-case core-focused gate below.

## Authority, compatibility, and nonclaims

Composition reads retained immutable compiler state and derives new immutable
Candidates and evidence in memory. It owns no filesystem write, persistent
cache, service-generation pivot, process execution, runtime test, source
commit, approval, Git, network, signing, deployment, payment, or publication
authority. Every report says `authority: false` and explicitly declines source
commit or publication authority.

The feature is additive. Semantic Transaction v1 intent, impact, review,
result, and evidence bytes are unchanged. Existing Project Candidate rebase,
merge, semantic-delta and source-review schemas and algorithms remain their own
versioned contracts. Canonical Semantic Workspace Revision, Semantic Workspace
Image, Project, managed Workspace, legacy CLI query, and frozen Project Agent
Transport v5 bytes remain unchanged.

This badge does not provide a universal multi-operation algebra, comments or
unrelated-trivia preservation, arbitrary source merge, general conflict proof,
behavioral equivalence, external-consumer compatibility, persistent service or
transport, MCP/LSP/editor route, source commit, managed `ACTIVE` pivot, or
publication workflow. It advances only composition of the admitted explicit
monomorphic non-main function display rename.

## Focused evidence

The focused integration evidence lives in
`tests/project_candidate/universal_semantic_transaction_composition.rs` as a
module of the existing Project Candidate harness. Its five cases cover
deterministic structural diff and exact replay; unrelated-drift rebase with
direct Candidate parity and exact replay; both explicit disjoint merge orders
with direct Candidate parity and replay; same-target, stale, tampered,
noncanonical, and cross-base rejection; no filesystem writes; and unchanged
Semantic Transaction v1, Candidate, and canonical workspace bytes.

The following focused command passed locally with five tests and no failures:

```sh
CARGO_TARGET_DIR=target/universal-semantic-transaction-composition-v1 \
  cargo test --locked -p semaprax --test project_candidate \
  universal_semantic_transaction_composition --no-fail-fast
```

The exact CLI projection passed four additional focused cases:

```sh
CARGO_TARGET_DIR=target/universal-semantic-transaction-composition-v1 \
  cargo test --locked -p semaprax --test workspace \
  universal_semantic_composition_cli --no-fail-fast
```
