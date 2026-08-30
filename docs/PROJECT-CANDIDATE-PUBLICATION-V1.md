# Project Candidate Managed Publication v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler maintainers and hosts explicitly publishing approved Project candidates.

This bridge publishes an independently replayed candidate through the existing
managed Workspace `ACTIVE` authority. It does **not** commit canonical Git
sources, rewrite original `.spx` files, publish a Project manifest, or make raw
path readers observe a transaction. The broader canonical-source commit goal
remains open.

## Separate host API

The functions are exported from `semaprax::project`:

```rust
pub fn prepare_candidate_publication(
    candidate: &ProjectCandidate,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
) -> Result<ProjectCandidatePublication, Vec<Diagnostic>>;

pub fn apply_candidate_publication(
    candidate: &ProjectCandidate,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
) -> Result<String, Vec<Diagnostic>>;
```

The opaque preparation exposes `to_json()`, `publication_digest()`, `proposal()`,
`workspace_change_evidence()`, and `candidate_workspace_revision()`. Preparation
is read-only and creates no proposal file, cache, staging object, or generation.
It does not initialize a workspace. An existing independently initialized
Semantic Workspace is required. Publication is a separate explicit host call;
there is no transport, CLI, automatic capsule import, or candidate method that
grants filesystem authority.

The root must be the exact absolute UTF-8 path of the independently authenticated
Project root, and the manifest argument must name its `semaprax.toml`. Existing
Project and Workspace path/handle authentication rejects aliases and drift.
The caller supplies both expected digests independently of the proof. Calling
an argument “approved” is a host contract, not a signature, consent detector,
or authorization service.

## Binding and replay

The host opens and verifies the entire raw Project manifest and source set.
Its Project revision and canonical manifest must equal the candidate's original
base, and the candidate may not change that manifest. The bridge then acquires
the existing permanent shared lock for preparation or exclusive lock for apply.
Both modes authenticate `ACTIVE` before candidate-history replay.

While that lock remains held, the bridge compares the whole managed source
inventory with the Project base: ordered paths, exact bytes, graph schemas,
source revisions, and source digests. The supplied Workspace revision must
match both `ACTIVE` and the candidate's original base Workspace revision.
Candidate history is independently replayed from the **freshly authenticated
Project snapshot**, including the ordinary Project, ownership, cleanup, and
admitted target checks. Its full canonical evidence and final digest must agree
with the candidate and the caller's expected approval digest.

The compiler derives the ordinary replacements-only
[Change-v1 proposal](SEMANTIC-WORKSPACE-CHANGE-V1.md). The existing Change builder
reconstructs its complete base/candidate workspace analysis and evidence under
the same lock. The candidate workspace revision must exactly match the freshly
replayed Project result. Apply compares the submitted outer proof with the
complete freshly reconstructed canonical bytes, then invokes the ordinary
Change-v1 typed and exact-byte evidence replay before any candidate generation
or staging object is created.

There are no replacement sources or HIR loaded from an untrusted capsule as
compiler state. Submitted proof bytes are bounded and compared directly with
the independently rendered result. Changed sources are compiler output from
candidate replay, not host-supplied publication edits.

## Proof and receipts

The proof schema is `semaprax.project-candidate-publication.v1`. It contains:

- compiler package and version;
- exact absolute Workspace root and Project manifest path;
- original/final Project and Workspace revisions;
- exact canonical Project manifest;
- approved candidate digest and complete canonical candidate evidence, including
  its full ordered change history;
- exact canonical Change-v1 proposal and evidence;
- limits and explicit nonclaims.

The proof is deliberately host-specific. Moving it to another otherwise
identical root requires new preparation. Strings embedding canonical child
artifacts preserve all their original bytes. Object keys are recursively sorted
lexically, arrays retain their defined order, and output is compact UTF-8 JSON
with exactly one terminal LF. The proof digest is:

```text
sha256("semaprax.project-candidate-publication.artifact.v1\0" ||
       little_endian_u64(proof_byte_length) || exact_proof_bytes_including_LF)
```

That digest is exposed by a getter and receipt, not embedded circularly in the
proof. The submitted proof is neither an approval token nor a signature.

Successful application returns
`semaprax.project-candidate-publication-application.v1`, identifying
`managed_generation_published`, both Workspace revisions, the candidate/proof
digests, and the existing `ACTIVE` pivot. It explicitly reports that original
source files are unchanged and no Git commit occurred. A receipt is descriptive
output, not reusable publication authority.

## Publication and failure boundaries

Only `workspace::commit_semantic_change_authority_with_hook` owns generation
creation and the sole `ACTIVE` replacement. The bridge rechecks held Project
inputs after replay, before candidate publication, at the existing final check
boundaries, immediately before `ACTIVE` replacement, and after replacement.
Workspace authentication, permanent locking, generation bounds, final checks,
and post-pivot validation remain unchanged.

Invalid approval, stale base, proof tampering, or replay failure reject before
candidate generation/staging. Original source files remain unchanged. A failure
later in the existing staging route can leave its bounded unpublished artifacts;
this bridge adds no rollback or cleanup authority. `SPX-G248` explicitly means
`ACTIVE` publication occurred before a later failure: do not assume unchanged
managed state or retry blindly. The host must inspect the managed snapshot.
Checks cannot make independently edited raw files and `ACTIVE` one atomic
transaction.

Because original raw sources remain unchanged, a second publication based on
that old raw Project is stale after the first pivot. Reusing the new managed
state as a new host Project base requires a separate explicit workflow; the
bridge does not silently synchronize Git files or rebind a retained candidate.

## Bounds and evidence

The unchanged Change-v1 domain permits **2–16 genuinely changed files**, each
at most 1 MiB, at most 4 MiB total replacement source, the same managed path set,
and supported base source Graph v10–v14 schemas. One-file changes and no-ops
reject; unchanged files are never inserted as padding. Unsupported newer graph
schemas reject through existing Change/Workspace diagnostics. This limitation
is not hidden by the broader candidate API.

The outer proof output/submission limit is 128 MiB, including its terminal LF;
this is an output bound, not a total heap bound. Existing candidate, Project,
Change-v1 analysis, evidence, and managed-storage bounds still apply. New
bridge diagnostics are `SPX-G245` for grammar/domain restrictions,
`SPX-G246` for proof output capacity, `SPX-G247` for stale/replay mismatch, and
`SPX-G248` for observed postpublication uncertainty. Existing diagnostics retain
their meaning and may propagate.

Authored, unrun tests in `tests/project_candidate_publication_v1.rs` cover
read-only preparation, deterministic proof, real managed publication with raw
source preservation, stale repeat apply, proof/approval/root substitution,
exclusive-lock ordering, raw Project drift, and single-file rejection. A unit
regression injects raw-source drift after the pivot and requires the new managed
revision together with `SPX-G248`. No local tests, compiler checks, long gates,
or publication calls were executed during implementation, as requested.

Canonical Git publication, arbitrary path-set changes, one-file adaptation,
newer graph-schema admission, target execution, signatures, general approval
policy, and full source-commit completion remain outside this version.
