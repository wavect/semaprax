# Project Candidate Assurance and Acceptance v1

Status: implemented bounded profile; local evidence only (see "Known
limitations" below). Composes [Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md)
(#183) and reuses the separate-act pattern
[Owned Workflow Approval v1](OWNED-WORKFLOW-APPROVAL-V1.md) (#128) established;
neither module is modified.

Audience: agents and reviewers who must decide whether one exact candidate is
ready to accept, and compiler contributors extending candidate evidence.

This closes [SPX-AI-030](https://github.com/wavect/semaprax/issues/129): give
agents and reviewers an exact, source/candidate-bound answer to what has been
checked, and an acceptance boundary a candidate cannot grant to itself. It adds
exactly two things: `ProjectCandidate::candidate_assurance_summary` and
`ProjectCandidate::grant_candidate_acceptance`, both implemented by
`src/project/candidate/candidate_assurance.rs`. It does not implement or
require an SMT solver, model checker, or proof kernel (those are the later,
independent [#184](https://github.com/wavect/semaprax/issues/184),
[#185](https://github.com/wavect/semaprax/issues/185),
[#186](https://github.com/wavect/semaprax/issues/186)); it does not
re-implement obligation derivation or the assurance lattice, and it does not
run project tests or execute a target.

## Bounded scope and non-goal

This is a compact, read-only join over an *existing* evidence artifact
(`semaprax.assurance-manifest.v1`), not a second competing evidence system. It
never calls `assurance_manifest::generate` itself: every envelope is supplied
by the caller (a test harness, a CLI wrapper once one exists, or a future
SMT/model-checking/proof-kernel producer) and independently re-verified and
rebound here. It is not a general multi-file cross-reference checker: each
supplied envelope is bound to exactly one candidate source path, and nothing
is inferred about relationships between files.

## Library API and binding

```rust
pub struct CandidateAssuranceInput<'a> {
    pub path: &'a str,
    pub envelope: &'a str,
}

impl ProjectCandidate {
    pub fn candidate_assurance_summary(
        &self,
        expected_candidate: &str,
        inputs: &[CandidateAssuranceInput<'_>],
    ) -> Result<String, Vec<Diagnostic>>;

    pub fn grant_candidate_acceptance(
        &self,
        expected_candidate: &str,
        inputs: &[CandidateAssuranceInput<'_>],
        minimum_class: AssuranceClass,
        proposer: &str,
        reviewer: &str,
    ) -> Result<String, Vec<Diagnostic>>;
}
```

`inputs` names, at most `MAX_CANDIDATE_ASSURANCE_INPUTS` (64) times, one
already-generated `semaprax.assurance-manifest.v1` envelope for one exact
candidate source path. `candidate_assurance_summary` authenticates the
candidate selector first (`self.require_candidate`), rejects a duplicate
input path, then for every input: independently replays the envelope with
`assurance_manifest::verify_envelope` (no filesystem access, no trust in the
caller's claimed validity), parses this exact candidate's own held source
text at that path with the compiler's own `crate::parse`, and recomputes
`crate::graph::revision` over it — the identical function
`assurance_manifest::generate` itself calls. Only when that recomputed
revision equals the envelope's own `payload.source.revision` is the
envelope's obligations joined into the summary. `payload.source.path` (a
caller-supplied display string) is never compared; the binding is the
recomputed revision over this candidate's own bytes, which is stronger.

## Why this cannot be satisfied by a stale, mismatched, or target-only fact

- **A previous-candidate or sibling-file envelope** carries a
  `source.revision` computed over different source bytes. Recomputing the
  revision from this candidate's own current bytes at the same path produces
  a different value, so the join fails closed (`SPX-G932`) before any
  obligation from that envelope is trusted.
- **A structural target-emission fact** (a Wasm or native emission check)
  cannot appear here at all: `verify_envelope` requires the exact
  `semaprax.assurance-manifest.v1` schema, so a differently-shaped report
  (for example `semaprax.project-candidate-test-report.v1`) is rejected
  before its content is even inspected.
- **A candidate source path with no supplied input** is listed under
  `sources_not_observed` in the joined summary, and `grant_candidate_acceptance`
  refuses to grant while that list is non-empty: an incomplete input set can
  never present as complete assurance.

## Honest disclosure of what is not yet derived

`assurance_manifest::generate` derives eight of the nine closed
`ObligationKind` tokens under their audited conditions. The joined summary
carries `kinds_not_yet_derived: ["architecture_law"]` in every summary:
that kind has no audited automatic producer. This field describes producer
coverage, not the obligations present in a particular source. An unobserved
candidate source still appears separately in `sources_not_observed`; neither
absence can be read as "verified". This is the exact failure case the owning
issue names first: "unsupported and unobserved targets remain explicit;
truncation cannot look like full assurance."

## No formal-proof status without real proof evidence

`assurance_manifest`'s own structural replay does not require a `proof_ref`
on an `smt_proved`/`model_checked`/`theorem_proved` method record (it only
forbids one on a *weaker* class). This join adds that check independently:
any obligation whose classification is one of those three formal-proof
classes, with no method among its records carrying a non-null `proof_ref`, is
listed under `unsupported_formal_claims`. `grant_candidate_acceptance` refuses
to grant while that list is non-empty, regardless of how strong the declared
`classification` string reads.

## An unrelated passing obligation never hides a failing one

The summary never collapses to one pass/fail boolean. Every obligation keeps
its own `id`, `declaration_id`, `kind`, and `classification`, sorted by `id`.
`grant_candidate_acceptance` computes `unmet_obligations` as the exact list of
obligation `id`s whose classification does not meet-or-dominate
`minimum_class` (using `assurance_manifest::dominates`'s own partial order,
never a simplistic total ordering) — a wrong postcondition's own obligation
id is reported on its own, and no other obligation's stronger classification
can remove it from that list.

## A candidate that edits visible tests cannot alter its own obligations

Every automatically derived obligation's `id` and `classification` traces
only to the compiler-checked `requires`/`ensures`/ownership facts of the
*production* declaration a `CandidateAssuranceInput` names — never to any
test file's content. A candidate is free to rewrite its own visible test file
to claim success; because that file's bytes never enter obligation derivation
for a different path's declarations, the joined summary for the unrelated
production file is byte-for-byte identical whether the test file lies or
not. `tests/project_candidate/candidate_assurance.rs`'s
`editing_the_visible_test_file_cannot_alter_the_production_obligations_or_acceptance`
proves this directly: two candidates differing only in `src/tests.spx`
content produce byte-identical `obligations`/`by_class` for `src/core.spx`.

## An independent acceptance boundary: a candidate cannot accept itself

`grant_candidate_acceptance` takes two caller-supplied identity strings,
`proposer` (whoever authored or is requesting acceptance for this candidate)
and `reviewer` (whoever is evaluating it), and refuses outright with
`SPX-G933` when they are equal, before evaluating anything else. This mirrors
[issue #104](https://github.com/wavect/semaprax/issues/104)'s ledger
requirement that "an agent under evaluation must not be able to edit its own
acceptance," applied structurally here: the function's own decision path has
no branch that can be reached when `proposer == reviewer`.

This is a string-identity check, not a cryptographic authentication system.
Matching every other evidence artifact in this codebase
(`OwnedWorkflowApproval`'s `approval_authority: false`,
`analysis_*_evidence`'s `publication_authority: false`), the returned record
carries `acceptance_authority: false`, `publication_authority: false`, and
`execution_authority: false`: it is proof that a named reviewer, distinct
from a named proposer, evaluated this exact candidate's independently
rebuilt assurance summary against a stated `minimum_class` — not a grant of
any authority. A real deployment binding `proposer`/`reviewer` to actual
distinct authenticated principals is an integration concern outside this
library, exactly as OwnedWorkflowApproval leaves the actual commit authority
to the existing managed-Workspace publication route.

`grant_candidate_acceptance` never accepts a caller-handed summary document:
it calls `candidate_assurance_summary` on the same `inputs` itself, so nothing
a candidate's own source (test files included) writes can reach the
acceptance decision except through that one independently bound and verified
path. `granted` is `true` only when every joined obligation meets or
dominates `minimum_class`, `sources_not_observed` is empty, and
`unsupported_formal_claims` is empty.

## Determinism, authority and diagnostics

Both functions are deterministic given identical inputs: no wall-clock time,
process id, or filesystem-ordering-dependent traversal reaches either report.
`candidate_assurance_summary`'s output is rendered through this candidate
module's own `wire::render` (recursively sorted object keys, one trailing
LF, bounded to `MAX_PROJECT_CANDIDATE_ASSURANCE_SUMMARY_BYTES`, 4 MiB).
`grant_candidate_acceptance`'s record is bounded to
`MAX_PROJECT_CANDIDATE_ACCEPTANCE_BYTES` (64 KiB). Both retain no image or
candidate and mutate no source; `candidate_retained`, `publication_authority`,
and `acceptance_authority`/`execution_authority` are always `false`.

Diagnostics: `SPX-G930` is a structural input error (a malformed envelope, an
unknown source path, or a duplicate input path); `SPX-G931` is a capacity
overflow (too many inputs, or final output exceeding its byte bound);
`SPX-G932` is the drift/rebind failure described above; `SPX-G933` is the
acceptance-boundary refusal (`proposer == reviewer`, or an empty/oversized
identity). None of these ever substitute a partial or empty report for a
failure.

## Implementation and tests

The implementation lives entirely in
`src/project/candidate/candidate_assurance.rs`. Regressions live as the
`candidate_assurance` module of the `project_candidate` integration harness
(`tests/project_candidate/candidate_assurance.rs`), covering: a genuine
envelope binding and joining correctly; a hand-tampered envelope (re-signed
so structural replay still accepts its shape) whose declared revision no
longer matches the candidate being rejected with `SPX-G932`; an envelope for
a path outside the candidate being rejected; duplicate input paths being
rejected; an externally supplied `theorem_proved` obligation with no
`proof_ref` being flagged under `unsupported_formal_claims` and blocking a
grant; a candidate being unable to accept itself (`proposer == reviewer`)
while a genuinely distinct reviewer can still grant the same inputs; and a
visible-test-file edit leaving the production obligations byte-identical.

## Known limitations

- **No CLI subcommand or transport route yet.** Both functions are
  library-only in this tranche, exactly like `assurance_manifest` itself.
  Wiring a CLI/MCP surface touches files outside this issue's file lease
  (`src/cli_driver.rs`, `src/cli/**`, `src/live_invocation/**`,
  `src/agent_interaction_schema/**`); the exact delta is future work for
  whichever worker owns that surface next.
- **Per-file, not whole-project.** Each `CandidateAssuranceInput` binds one
  source path; there is no automatic discovery of which candidate paths need
  an envelope beyond reporting the gap in `sources_not_observed`. Generating
  the envelopes themselves (one `assurance_manifest::generate` call per
  candidate source file, against real bytes on disk) is the caller's
  responsibility.
- **A library/provider module can never be one of those candidate source
  paths (#230).** `assurance_manifest::generate` requires the exact file it
  is given to stand alone as a runnable, import-free program (`fn main() ->
  i64`, no `module_uses`; see "Known limitations" in
  [Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md)), while a project forbids
  `main` on any source but its entry and test modules. No file satisfies
  both, so a plain library/provider module can only ever appear under this
  module's `sources_not_observed`, never as a source with its own envelope.
  This is by design, not a defect this module works around: it reports the
  gap honestly (`sources_not_observed` is non-empty and acceptance is
  withheld while it is) rather than silently treating a library module's
  obligations as covered.
- **Identity is caller-asserted.** `proposer`/`reviewer` are plain strings;
  this library enforces only that they differ, never that either names an
  actual, authenticated, distinct principal. See "An independent acceptance
  boundary" above.
- **`unsupported_formal_claims` covers only the three formal-proof classes.**
  It does not re-derive whether a `runtime_guarded` or `compiler_proved`
  claim is itself correct; that trust is inherited from
  `assurance_manifest::generate` and the compiler passes it already reused
  (`verify::verify`, `wasm::emit_contract_guard`).
