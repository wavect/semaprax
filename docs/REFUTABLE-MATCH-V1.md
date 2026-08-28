# Refutable Match v1 (Literal Patterns + Guards)

Audience: language users, tool authors, and compiler contributors.

Status: Partial — this tranche adds refutable `match` arms over an explicitly
admitted Copy-scalar profile, evidenced by `tests/refutable_match_v1.rs` and
`examples/refutable_match.spx`. The broader matching rows of
[COMPLETION-MATRIX.md](COMPLETION-MATRIX.md) are unchanged by this document
alone.

## Objective

SEMAPRAX matching previously admitted only exhaustive copy-variant matches
and irrefutable Copy-record destructuring. This tranche adds the smallest
refutable slice that keeps every cleanup, ownership, and backend invariant
intact:

```text
match <scalar> {
    0 => <value>,
    -1 | -2 => <value>,
    7 if scrutinee > 3 => <value>,
    n => <value>,
}
```

Match remains an **expression** producing exactly one arm value; evaluation
order is arm order, and the scrutinee evaluates exactly once.

## Syntax and canonical form

New pattern forms, all parsed only in match-arm position:

- Integer literals (`0`, `-5`, `9u8`, `1i32`) with sign folding at parse time.
  Suffix typing rules are identical to expression literals because the same
  lexer tokens feed both; `-9223372036854775808` stays unrepresentable exactly
  as in the expression grammar (`SPX-P003` at the lexer).
- Bool literals (`true`, `false`) and char literals (`'a'`, `'\u{fc}'`).
  Float and string literal patterns never parse (`SPX-P206`) — bit-exactness
  hazards keep floats outside v1 by design.
- Or-patterns `a | b | c` over literal alternatives of one type. The lexer's
  single `|` becomes a token consumed only here; nesting or non-literal
  alternatives are rejected with `SPX-M105`.
- Irrefutable whole-scrutinee bindings (`n => ...`), immutable and `Value`
  owned.
- Guards: `pattern if guard => value`. The guard is an ordinary bool
  expression resolved under the arm's pattern bindings plus outer locals. It
  evaluates once, after its pattern matched and before any part of the arm
  value; a false guard falls through to the following arms.

The canonical formatter renders match arms inline exactly as pre-feature
output did for aggregate matches; guarded arms render `pattern if guard =>
value`. Programs without refutable-match syntax format byte-for-byte
identically to pre-feature output.

## Semantics

1. The scrutinee evaluates once into backend-owned staging (a C temporary, a
   dedicated Wasm local, or one interpreter value); every arm test re-reads
   that single evaluation.
2. Arms test in source order. A selected arm whose guard passes produces the
   arm value; a failed pattern or a false guard falls through. Later arms are
   reachable only while no earlier arm selected; unreachable-arm detection is
   NOT performed in v1 (deliberate nonclaim).
3. Guards may fail (checked arithmetic, propagated calls) exactly like any
   expression; failure selection stays sticky and poison-preserving.
4. Arm values must agree on type and ownership across all arms
   (`SPX-T259`).

## Admitted profile (v1 only)

Refutable matches require a Copy scalar of exactly `i64`, `i32`, `u8`,
`char`, or `bool`. Literal patterns compare against exactly their own
scrutinee type (`SPX-T255`); there is no numeric widening anywhere in v1.

Every refutable match REQUIRES one trailing irrefutable guard-free catch-all
arm — `_` or a binding (`SPX-T257`). Exhaustive bool matches written as
`true => ..., false => ...` therefore still need a catch-all in v1; this is a
deliberate simplification of exhaustiveness reasoning, not a claim that such
matches are non-exhaustive.

Everything else is rejected fail-closed rather than approximated:
guards/literal/or/binding patterns against record, variant, class, string,
float, or generic scrutinees (`SPX-T254`), aggregate patterns on scalar
scrutinees (`SPX-H001`), and refutable-match syntax inside generic function
templates (materialization rejects all matches there, unchanged). While-loop
bodies keep their existing admission profile: match expressions inside loop
conditions/bodies remain rejected exactly as before this tranche, so
`while { match ... }` nesting is a nonclaim; the reverse nesting (loops as
arm-value statements) is admitted and evidenced.

## Cleanup-plan contract

The cleanup CFG stays acyclic. An admitted refutable match linearizes one
decision pass mirroring the while model:

- each non-final arm authenticates one additive
  `EdgeCondition::ArmSelected { scrutinee, arm, selected }` edge pair;
- a guard lowers as an ordinary bool expression whose
  `EdgeCondition::BooleanResult` joins route to the arm value or to the next
  decision block;
- every path joins at one block, and the builder fails closed unless joined
  owned liveness equals decision-entry liveness (`Refutable Match v1`
  admission makes this equality exact);
- binding arms bind Copy scalars, so no new slots, transitions, or finalizers
  exist for any admitted program.

Plans for programs without refutable syntax are byte-identical to pre-feature
output (schema selection below proves the gating; the shared pre-feature
graph pin proves the bytes). CleanupPlan v2 remains canonical; no plan-schema
bump was needed because the new edges carry no ownership events.

## Graph projection

Graph serialization gains additive node/attr spellings — `"kind":
"literal_pattern"`, `"kind": "or_pattern"`, `"kind": "binding_pattern"`,
per-arm `"guard"` objects, and `"exhaustive":false` — plus program-level
schema **Graph v16**, selected above the whole v10–v15 lattice exactly when
an authenticated refutable node exists (guard, literal, or-pattern, or
binding arm). Wildcard-free programs without refutable syntax keep their
exact previous schema and bytes (pinned digests, including the shared
pre-feature scalar pin and a pinned aggregate corpus carrying variant
matches, record patterns, and cleanup projections).

Patch/evidence/workspace flows stay fail-closed: their verifiers
authenticate only `semaprax.graph.v10`–`v14`, so generation refuses both
additive schemas up front (`SPX-G410`).

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-P206` | Malformed literal/negative pattern syntax at parse time. |
| `SPX-T254` | Refutable-match construct on a non-admitted scrutinee. |
| `SPX-T255` | Literal pattern type differs from the scrutinee type. |
| `SPX-T256` | Guard is not exactly `bool`. |
| `SPX-T257` | Refutable match lacks a trailing irrefutable guard-free catch-all. |
| `SPX-M105` | Or-pattern misuse (non-literal, mixed-type, or empty alternatives). |
| `SPX-T259` | Match arms disagree on the result type. |
| `SPX-G410` | Evidence/patch generation from a Graph-v15/v16 source (extended message, unchanged code). |

Unknown names inside guards reuse the established diagnostics; immutable-
target and assignment errors are untouched.

## Layer behavior

- **Parser/AST**: `MatchArm.guard`, `MatchPattern::{Literal, Or, Binding}`,
  and `PatternLiteral`; lexer emits `TokenKind::Pipe` for single `|`.
- **Canonical formatter**: inline arms with `pattern if guard => value`;
  budgets account for guards; non-refutable bytes unchanged (frame-order
  regression covered by the example gate).
- **Resolver/HIR**: iterative resolver gains `ScalarMatchNext`/
  `ScalarMatchAfterArm` frames; the recursive oracle twin mirrors them
  producing identical identities; generic templates reject all matches
  (unchanged).
- **Verifier twins**: `source_verify` gains a `ScalarMatchState` machine
  (iterative) plus the recursive oracle branch; HIR validation re-checks the
  full admission structurally against hostile HIR.
- **Cleanup plans**: builder frames + `lower_scalar_match` recursive twin;
  replay authenticates `SkeletonObservation::ArmSelected`, complementary
  pairs, census weights, scenario-driven execution
  (`CleanupScenario::arm_selections`), and hostile-HIR skeleton expectations.
- **Native C11**: staged scrutinee temporary, sequential
  `if (!matched && (<literals>))` tests, inner guard branch selecting before
  the value, defensive no-arm runtime-invariant check; O0/O2 identical.
- **Wasm**: core lane stages the scrutinee into a per-expression local and
  emits nested reject blocks with `br_if 0` fall-through and a result-carrying
  `$done` block; the aggregate lane shares the shape over its own planned
  locals. Binding arms alias the staging local (core) or planned locals
  (aggregate). Checked-arithmetic failures reuse existing traps/host imports.
- **Interpreter**: structural admission scan admits scalar decision chains,
  evaluation charges fuel per node, guards evaluate once after selection, and
  fuel exhaustion reports the existing fail-closed outcome.

## Evidence

`tests/refutable_match_v1.rs` pins canonical round-trips, deterministic
Graph-v16 serialization plus untouched v10/v13 selections and the historical
pre-feature byte pins, slot-free/finalizer-free plans with explicit
`ArmSelected` structure, stable regressions for every new diagnostic, native
C11 O0/O2 agreement across the full corpus (negative i64 dispatch, u8
boundaries, char routes, guarded bindings, or-pattern fall-through, nested-if
guards, loops inside arm bodies), Node/Wasm equivalence on `main()`, full
interpreter agreement including recursion through guards, and fuel-exhaustion
fail-closed behavior. `examples/refutable_match.spx` exercises the surface
under the example check/fmt gates.

## Non-claims

This tranche does not claim float or string literal patterns, range patterns
(`0..=9` — deliberately not implemented; the parser rejects `..` as
unexpected tokens and no range admission exists), exhaustive-bool special
casing, or-patterns over bindings/aggregates, nested or-patterns, guards or
literal patterns on record/variant/class/string scrutinees, refutable matches
inside while conditions/bodies (the while profile is unchanged), refutable
matches in generic templates, unreachable-arm detection, match-failure
witness reporting beyond the codes above, patch/workspace-evidence admission
for Graph-v16 sources, property-test/hygienic generation over the new
patterns, or any hosted-CI promotion beyond the gates listed above.
