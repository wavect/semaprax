# Requirement Traceability v1

Status: bounded initial slice of a larger product goal; the completion
matrix owns product status. Composes [Assurance Manifest
v1](ASSURANCE-MANIFEST-V1.md) (#183) exactly the way [Project Candidate
Assurance and Acceptance v1](PROJECT-CANDIDATE-ASSURANCE-ACCEPTANCE-V1.md)
(#129) does; neither module is modified.

Audience: agents and reviewers who must decide whether a requirement is
technically satisfied by exact, current evidence, and compiler contributors
extending requirement/intent traceability toward the full scope of #202.

This is a deliberately narrow slice of
[#202](https://github.com/wavect/semaprax/issues/202) ("Add first-class
requirement, intent, scenario, and evidence traceability objects"): **binding
a requirement's acceptance criteria to exact assurance subjects that fail
closed when the subject changes**, not the full requirement/intent/scenario
object model #202 describes. See "Explicitly out of scope" below for the
rest of #202 this tranche does not attempt.

## The problem this closes

A requirement tracked against an *approximate* subject — a file path, a
declaration's display name, a revision string trusted at face value — is not
really tracked: any of those can drift silently and the requirement would
keep reporting whatever it last observed. This module binds a requirement
criterion to an **exact assurance subject**:
[`crate::assurance_manifest::obligation_id`](../src/assurance_manifest/obligation.rs)
— itself a length-prefixed, collision-resistant identity derived from a
declaration's persistent `stable_id`, a closed obligation kind, and a
structural locator — evaluated only against a
`semaprax.assurance-manifest.v1` envelope that
[`evaluate_requirement`](../src/requirement_traceability.rs) has itself just
independently re-verified and rebound to the *current* bytes on disk at the
criterion's exact source path. A criterion is never satisfied by a cached,
handed-in, or self-reported verdict.

## Library API

```rust
pub struct RequirementCriterion { /* source_path, obligation_id, minimum_class */ }
impl RequirementCriterion {
    pub fn new(source_path, obligation_id, minimum_class: AssuranceClass) -> Result<Self, Diagnostic>;
}

pub struct Requirement { /* id, title, criteria */ }
impl Requirement {
    pub fn new(id, title) -> Result<Self, Diagnostic>;
    pub fn with_criterion(self, criterion: RequirementCriterion) -> Result<Self, Diagnostic>;
}

pub struct EvidenceInput<'a> {
    pub source_path: &'a str,
    pub envelope: &'a str,
}

pub fn evaluate_requirement(
    requirement: &Requirement,
    evidence: &[EvidenceInput<'_>],
) -> Result<String, Diagnostic>;
```

`title` is explanatory natural-language text. `evaluate_requirement` never
reads it: it cannot affect the derived satisfaction verdict, matching #202's
explicit "natural language never substitutes for machine evidence"
requirement and the wider repository invariant that natural-language goal
text can carry instructions unrelated to trusted criteria.

## How a criterion is evaluated

For each distinct `source_path` a requirement's criteria reference,
`evaluate_requirement` finds the matching `EvidenceInput` (refusing
`SPX-Z403` if evidence names one path more than once — ambiguous evidence is
never silently resolved by picking one) and calls
[`assurance_manifest::verify_envelope_against_source`](ASSURANCE-MANIFEST-V1.md)
on it. That call independently structurally replays the envelope end to end
and rebinds its embedded `source.sha256` to the exact current bytes at that
path — never the envelope's self-reported path or revision. One of three
things happens:

- **The envelope's bytes have drifted from current source** (the
  `SPX-Z104` drift diagnostic): every criterion naming that source path is
  reported `stale`, and a requirement with any `stale` criterion is `stale`
  overall — the highest-priority verdict, because a drifted subject
  invalidates conclusions about it before anything else is meaningful.
- **The envelope is otherwise structurally malformed** (any other
  `crate::assurance_manifest` code, `SPX-Z101`..`SPX-Z103`): this is a
  caller/input error, not evidence content, and is propagated unchanged
  rather than silently downgraded to a criterion verdict.
- **The envelope verifies and rebinds cleanly**: the exact `obligation_id`
  is looked up in its `payload.obligations`. Absent, it is `dangling`
  (unmet link → the requirement is `failed`, unless a stronger verdict
  already applies). Present, its current `classification` is compared
  against the criterion's `minimum_class` using
  [`assurance_manifest::dominates`](ASSURANCE-MANIFEST-V1.md)'s own partial
  order — never a simplistic total ordering. Meeting or exceeding the
  minimum through `assumed`/`attempt_inconclusive` evidence is reported
  `assumed`, never `satisfied`: an explicit human assumption must never
  read as machine-checked proof. Otherwise meeting or exceeding the minimum
  is `satisfied`; falling short is `unmet`.

No evidence supplied for a criterion's source path at all is `unevaluable`
— never a default pass or fail.

## The requirement-level verdict

One requirement aggregates its criteria conservatively, worst case first:

1. Any criterion `stale` → the requirement is `stale`.
2. Else any criterion `dangling` → `failed`.
3. Else any criterion `unevaluable` → `unevaluable`.
4. Else, among the remaining `satisfied`/`assumed`/`unmet` mix: all
   `satisfied` → `satisfied`; no `unmet` but some `assumed` → `assumed`;
   both `satisfied`/`assumed` and `unmet` present → `partial`; only `unmet`
   → `failed`.

A requirement is `satisfied` only when every one of its criteria currently
has qualifying, freshly re-verified evidence — matching #202's "compute
satisfaction conservatively" requirement directly.

## Rejecting ambiguous and duplicate links up front

`Requirement::with_criterion` refuses (`SPX-Z403`) an exact duplicate: a
second criterion naming the identical `(source_path, obligation_id)` pair
already attached is an ambiguous link (which `minimum_class` would govern?)
and is never silently merged or overwritten.
`evaluate_requirement` similarly refuses ambiguous *evidence*: two
`EvidenceInput` entries naming the same `source_path` are rejected rather
than one being silently preferred. `evaluate_requirement` itself refuses
closed (`SPX-Z401`) when a requirement carries zero criteria: there is
nothing machine-checkable to evaluate, so nothing is ever reported
"satisfied" by default.

## Determinism

`evaluate_requirement` is deterministic given identical inputs: no
wall-clock time, process id, or filesystem-ordering-dependent traversal
reaches the report. Criteria render sorted by `(source_path, obligation_id)`
regardless of the order they were attached in, matching the sort-before-
render convention `assurance_manifest::render` and `candidate_assurance`
already use.

## Diagnostics

- `SPX-Z401` — invalid input: an empty or over-bound requirement
  id/title, an empty criterion `source_path`/`obligation_id`, or a
  requirement with no criteria passed to `evaluate_requirement`.
- `SPX-Z402` — a capacity bound exceeded: `MAX_REQUIREMENT_CRITERIA` (256)
  per requirement, `MAX_EVIDENCE_INPUTS` (256) per call, or the rendered
  report exceeding `MAX_REQUIREMENT_REPORT_BYTES` (1 MiB); fail closed,
  never truncated.
- `SPX-Z403` — an ambiguous input this module refuses to silently resolve:
  a duplicate `(source_path, obligation_id)` criterion, or evidence naming
  one source path more than once.

A structurally malformed or drift-rejected assurance envelope keeps its own
`crate::assurance_manifest` code (`SPX-Z101`..`SPX-Z104`); only the drift
code is caught here and downgraded to a per-criterion `stale` verdict rather
than aborting the call, because drift is the expected "the subject changed"
case this module exists to report rather than treat as a hard error.

## Exact nonclaims

The rendered report carries this array verbatim:

```text
not_a_language_syntax_feature
not_publication_or_execution_authority
not_human_approval_or_policy
no_target_execution
no_project_test_discovery_or_execution
read_only_no_source_changes
natural_language_title_is_not_evidence
```

## Two recorded gaps this tranche inherits rather than works around

- **[#230](https://github.com/wavect/semaprax/issues/230):**
  `assurance_manifest::generate` requires a standalone `fn main() -> i64` in
  the exact file it is given, while `SPX-G172` forbids `main` on any
  non-entry, non-test source. A plain library module can never itself
  receive an `assurance-manifest.v1` envelope in v1 — it can only appear
  under a joining consumer's `sources_not_observed` (see
  `candidate_assurance_summary`). Consequently, **a requirement criterion in
  this tranche can only ever name an obligation belonging to a source file
  that is itself an executable entry module** (or is evaluated standalone,
  outside a managed project, the way this module's own tests do). If #202's
  full goal of covering arbitrary library surface is required, #230 blocks
  it structurally; this tranche does not attempt a workaround, because doing
  so would mean re-deriving obligations outside `assurance_manifest`'s own
  producer — exactly the "do not create a parallel source of truth"
  prohibition in the owning issue's rules.
- **[#184](https://github.com/wavect/semaprax/issues/184):** `render.rs`'s
  `nonclaims` array unconditionally asserts `no_smt_solver_invoked`, and
  `ExternalRecords` cannot merge a second method into an obligation
  `derive.rs` already populated (fails closed with `SPX-Z101`). This
  constrains how a future SMT/model-checking/proof-kernel producer can
  strengthen an obligation this module's criteria already reference; no
  workaround is attempted here either, for the same reason.

## Explicitly out of scope (left to the rest of #202)

- **No source or canonical-syntax representation.** `Requirement` and
  `RequirementCriterion` are library values only; there is no `.spx` syntax,
  parser, resolver/HIR, or semantic-graph node for a requirement in this
  tranche. #202's "Source or adjacent canonical syntax/data for requirements
  and scenarios" remains open.
- **No Intent or Scenario objects.** Only the requirement/criterion/evidence
  shape exists here.
- **No candidate delta.** #202 asks for "candidate delta showing impacted
  requirements, stale evidence, added/removed implementation links,
  assurance weakening, and required review." This tranche's `stale` verdict
  is a *point-in-time* check against currently supplied evidence, not a
  before/after delta between two candidate revisions the way
  [`assurance_manifest::delta`](ASSURANCE-MANIFEST-V1.md) or
  `candidate_assurance` compare two states. Composing this module with
  `assurance_manifest::delta` over two evaluation runs is future work.
- **No cross-requirement dependency graph.** #202 names circular requirement
  dependencies as a failure case to guard against; this tranche has no
  requirement-to-requirement links at all (only requirement-to-obligation),
  so that failure mode does not yet arise, but neither is it solved.
- **No author/reviewer/approval record type.** #202 asks for
  "author/reviewer/approver records as evidence/decision data." This
  tranche does not add one; `candidate_assurance`'s
  `grant_candidate_acceptance` already establishes the separate-act pattern
  (`OwnedWorkflowApproval`, `proposer != reviewer`) a future requirement
  approval record should reuse rather than reinvent.
- **No CLI/MCP/SDK exposure.** Library-only, exactly like
  `assurance_manifest` and `candidate_assurance` before it. Wiring a
  CLI/transport surface touches files outside this issue's file lease
  (`src/cli_driver.rs`, `src/cli/**`); the exact delta is future work for
  whichever worker owns that surface next.
- **No path canonicalization.** `source_path` matching between a criterion
  and its evidence is exact byte-for-byte string equality, matching
  `crate::assurance_manifest::verify_envelope_against_source`'s own
  filesystem-path handling; `./a.spx` and `a.spx` are different subjects
  here. This is a deliberate fail-closed default (ambiguity is rejected, not
  heuristically resolved), not an oversight, but it means a caller must
  supply criteria and evidence with identical path spellings.

## Implementation and tests

The implementation lives entirely in
[`src/requirement_traceability.rs`](../src/requirement_traceability.rs) plus
its `tests` submodule
(`src/requirement_traceability/tests.rs`), covering: construction bounds
(empty/oversized id, empty source_path/obligation_id); duplicate-criterion
and duplicate-evidence-path rejection; a satisfied criterion against real,
freshly generated evidence; an unmet criterion (an achieved class that does
not dominate a stricter required minimum); a mixed satisfied/unmet
requirement reported `partial`; a dangling reference to an obligation id
absent from current evidence; a criterion with no supplied evidence reported
`unevaluable`; an externally supplied `assumed` obligation reported
`assumed`, not `satisfied`; report determinism across repeated calls; and —
the crux of "exact assurance subjects" — a criterion evaluated `satisfied`
against unchanged source, the identical requirement and identical
(unregenerated) envelope evaluated again after the file's bytes change on
disk reported `stale`, and a freshly regenerated envelope against the new
bytes reported `satisfied` again (proving the pipeline was never broken,
only the stale evidence correctly refused). A companion negative control
confirms the drift is detected by the documented `SPX-Z104` code specifically
(not some earlier structural check), and a further case confirms a non-drift
envelope malformation is propagated rather than silently downgraded.

## Known limitations

- **Per-criterion, not whole-project.** Like `CandidateAssuranceInput`, each
  criterion binds one source path; there is no automatic discovery of which
  paths a requirement should reference. Generating the underlying
  `assurance-manifest.v1` envelopes is the caller's responsibility.
- **`source_path` is caller-asserted, matched exactly.** See "Explicitly out
  of scope" above.
- **Identity is caller-asserted.** `Requirement::id` is a plain, caller-
  supplied string with no cross-call registry enforcing global uniqueness;
  nothing here checks two different calls did not reuse the same id for a
  different requirement.
- **Inherits `assurance_manifest::generate`'s narrow automatic derivation.**
  Only `precondition`, `postcondition`, and `ownership_parameter` obligations
  exist to reference today (`ownership_result`, `effect`, `exhaustiveness`,
  `resource_cleanup`, `architecture_law`, and `generated_interface` need an
  `ExternalRecords` producer that does not yet exist); see "Obligation
  derivation" in [Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md).
