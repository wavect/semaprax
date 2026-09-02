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

## Bounded compiler-derived repair classes

`semaprax.project-candidate-repair-catalog.v1` offers at most one proposal per
attempt. `retag_integer_literal_to_retained_return_type` applies only when:

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

`borrow_owned_byte_field_without_staging` addresses an actual `SPX-T266`
rejection of a typed `replace_function_body` intention. Inside byte-view
constructors it replaces a value projection of an existing lexical root with
the corresponding [direct field place](PROJECT-FIELD-PLACE-CONSTRUCTOR-V1.md).
The stable field ID and lexical root remain unchanged. Calls, constructors,
and other computed projection bases are not converted into named roots.

The matched node is exactly a `builtin_call` to `core.bytes.as-slice` with
one `project` argument whose `base` is a closed `place` constructor. An absent
or empty projection `type_arguments` list is accepted; nonempty arguments
are outside the flat monomorphic borrow profile. The traversal visits only
expression positions of the existing constructor grammar, including lexical
bindings, conditionals, calls, aggregate fields, updates and match arms. It
rewrites all matching sites in traversal order into one proposal; an unsupported
projected byte-view base makes that proposal unavailable. Pattern, type and
binder metadata are not interpreted as expressions.

The description includes `diagnostic_code: SPX-T266`, `replacement_count`, and
ordered `replacements` containing each unchanged `field` ID and `root` name,
alongside the common complete change, exact selector, validated candidate
revision, admission basis and no-authority facts. These descriptors identify
the transformed typed inputs, not authenticated source-span locations.

The repair removes the projection constructor's temporary so that
`bytes_as_slice` can borrow the original field storage. It does not extend the
[source borrowing profile](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md): the ordinary
constructor authenticates the root's exact nominal owner, and source admission
still requires a direct owned `Bytes` field, a live owned root, valid loan use,
and complete ownership, cleanup, contract, effect, and target checks. A matching
diagnostic or syntactic pattern alone never yields an advertised repair. The
complete transformed body must pass normal candidate admission first.

This class intentionally changes an invalid proposed body's ownership behavior;
it does not claim equivalence to executable rejected source. Diagnostic spans
are not mapped onto predecessor HIR. Other body expressions retain their typed
construction and evaluation order, and no source or compiler rule is relaxed.

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
missing-name, API-shape, and general multi-error repair remain outside these
classes.

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
at most one full proposed repair apply per discovery/selection call. The
byte-field walk additionally enforces the existing 4,096 expression-node and
depth-64 limits. Ordinary constructor accounting, including implicit generated
nodes, still applies during full admission. The public `retained_report_bytes()`
conservatively sums attempt and private predecessor
report bytes for future registry admission; it is not a total HIR-memory bound.
The byte-field proposal also checks that its history-preserving repair selector,
including the nested rejected body, fits ordinary Semantic Change bounds before
advertising it. A candidate that fits only as an ordinary body edit does not
produce an unusable repair-history selector.

`SPX-G241` covers attempt/repair grammar, `SPX-G242` report capacity, and
`SPX-G243` stale or unavailable attempt/repair selectors. Ordinary digest/change
and compiler diagnostics remain unchanged where delegated.

This original attempt API has no source authority. The additive
[Project Diagnostic Change v1](PROJECT-DIAGNOSTIC-CHANGE-V1.md) now supplies a
history-preserving `repair_diagnostic` SemanticChange kind for these repair
classes; its catalogue object is `semantic_change_intent`. Rejected-attempt
persistence and automatic repair execution remain absent. Hosts must apply source
pre/post authentication, registry/response bounds, and explicit capability
policy before exposing it. The API itself reads only retained compiler state;
it does not acquire filesystem, build, test, process, network, or source-write
authority.

Typed requests and change-catalogue descriptors expose both classes. The full
repair report is included in the selected v5 response schema bundle, using
closed proposal alternatives and the compiler-owned recursive expression/change
schemas. [Typed response clients](IMAGE-TYPED-RESPONSE-CLIENTS-V1.md) validate
those structures before exposing concrete language types. This describes report
shape; only ordinary compiler replay establishes repair admission and identity.

[Focused authored tests](../tests/project_candidate/diagnostics.rs) cover
exact diagnostic retention, source/target binding, successful full-admission
numeric repair, unsupported and out-of-range cases, stale selectors, unchanged
predecessors/source, accepted outcomes, and structural input rejection. Tests
were not run for this change.

[Field-borrow repair regressions](../tests/project_candidate/field_borrow_repair.rs)
are also authored and unrun. They exercise actual projected-view rejection,
direct-field repair, nested composition, unsupported bases and owner mismatch,
remaining-invalid candidates, exact repair history and stale/replay boundaries.
