# Candidate Attempts and Typed Diagnostic Repair v1

Audience: compiler contributors, agent builders, and reviewers.

Status: additive library implementation with focused authored, unrun tests.
No test-execution, hosted-validation, or general diagnostic-repair claim.

A rejected semantic intention remains queryable through a diagnostic record,
without treating invalid source as a Project revision, candidate, or checked
image. The predecessor remains an immutable, fully admitted `ProjectCandidate`.
The existing `apply` API and every source/invariant gate are unchanged.

## Library API

`ProjectCandidateAttempt::apply(base: Arc<ProjectCandidate>, expected_candidate,
intent: &Value)` first checks the exact base digest and ordinary bounded
`SemanticChange::new` grammar. It then calls normal full candidate apply and
returns one of:

- `ProjectCandidateAttemptOutcome::Accepted(Arc<ProjectCandidate>)` with the
  same candidate an ordinary successful apply produces.
- `ProjectCandidateAttemptOutcome::Rejected(Arc<ProjectCandidateAttempt>)`
  containing the exact canonical change, verified predecessor, and diagnostics
  emitted by that failed apply.

Stale base bindings and oversized structural inputs are outer errors, not
retained attempt objects. Resource failure while constructing a bounded report
also remains an outer error; no partial diagnostic report is returned.

An attempt exposes `attempt_digest()`, `to_json()`, digest-bound
`summary(expected_attempt)` and `repair_catalog(expected_attempt)`, plus
`repair_diagnostic(expected_attempt, repair_id)`. Summary is compact; the
complete report is `semaprax.project-candidate-attempt.v1`, canonical compact
JSON plus one LF. It binds base candidate and Project revisions, exact change,
ordered diagnostics, and source/target provenance from the verified predecessor
semantic index where that target exists. Unknown or oversized target selectors
have null provenance; the exact attempted intention remains in the report.

Each diagnostic preserves code, severity, message, optional path/span, and help.
Diagnostic offsets describe the failed constructor or uncommitted candidate
source: they must not be interpreted as checked predecessor expression spans.
The separate target provenance carries the predecessor source revision/digest,
path/module, declaration kind, identity origin, and owner. No invalid generated
source, HIR, revision accessor, materialization token, or publication authority
is exposed. Keeping diagnostics does not make an invalid intention admissible.

## One bounded compiler-derived repair class

`semaprax.project-candidate-repair-catalog.v1` offers at most one class:
`retag_integer_literal_to_retained_return_type`. It applies only when:

1. the failed intention is exactly `replace_function_body` with one closed
   integer literal constructor (`i64`, `i32`, `u8`, or `usize`);
2. the target has a retained function return type in that same integer set;
3. the original numeric value is valid for its original literal type, the
   expected return type differs, and the exact value fits the expected type;
4. a replacement of only the literal's type tag succeeds through ordinary
   full `ProjectCandidate::apply` against the exact predecessor.

The compiler derives the expected type from retained HIR, preserves the exact
integer value, constructs a normal `replace_function_body` change, and validates
it before advertising the proposal. The catalogue includes that complete
change, typed basis, and validated candidate digest. It does not guess a default
value, rename a symbol, insert a missing expression, coerce booleans, truncate
integers, remove contracts, or weaken ownership/effect/identity constraints.

Selecting a repair re-derives it and repeats full candidate admission. Only the
matching digest selector returns the newly validated candidate; the rejected
attempt and predecessor remain unchanged. No source is written. `tests: not_run`
is explicit: compiler admission does not establish test success, target runtime
behavior, contract satisfaction on all inputs, or correctness of the user's
larger intended change.

Every unsupported case returns an empty repair array and a machine-readable
availability reason. The legacy `SPX-S103` repair assigns a persistent ID to an
automatic-ID function through `breaking_identity_rebase`. Automatic identities
may exist in otherwise checked sources, but existing candidate operations
require explicit targeted identities and preserve existing stable identities.
This API therefore does not import that legacy identity-changing repair or its
path-based A0 authority. General type, ownership, capability, missing-body,
missing-name, API-shape, and multi-error repair remain outside this class.

## Binding, limits, and integration boundary

The attempt digest hashes domain `semaprax.project-candidate-attempt.v1` plus
NUL, a little-endian u64 byte length, and exact complete report bytes including
LF. A repair digest uses domain `semaprax.project-candidate-typed-repair.v1`
plus NUL and the same length framing over canonical JSON containing attempt
revision, repair class, and complete derived change. Digests select content;
they are not signatures, approvals, capabilities, or externally trusted state.

Intent limits remain the ordinary 1 MiB/8,192-node/depth-64 bounds. A rejected
attempt admits at most 256 diagnostics and 1 MiB of cumulative diagnostic text;
its complete rendered report is at most 2 MiB, including escaped JSON. There is
at most one full proposed repair apply per discovery/selection call. The public
`retained_report_bytes()` conservatively sums attempt and private predecessor
report bytes for future registry admission; it is not a total HIR-memory bound.

`SPX-G241` covers attempt/repair grammar, `SPX-G242` report capacity, and
`SPX-G243` stale or unavailable attempt/repair selectors. Ordinary digest/change
and compiler diagnostics remain unchanged where delegated.

This tranche is library-only. No Image Agent Protocol attempt registry,
`repair_diagnostic` SemanticChange wire kind, CLI, rejected-attempt persistence,
or automatic repair execution is added. Future hosts must apply source
pre/post authentication, registry/response bounds, and explicit capability
policy before exposing it. The API itself reads only retained compiler state;
it does not acquire filesystem, build, test, process, network, or source-write
authority.

[Focused authored tests](../tests/project_candidate_diagnostics_v1.rs) cover
exact diagnostic retention, source/target binding, successful full-admission
numeric repair, unsupported and out-of-range cases, stale selectors, unchanged
predecessors/source, accepted outcomes, and structural input rejection. Tests
were not run for this change.
