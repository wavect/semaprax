# Finite Structured Law v1 (LAW-07 draft)

Status: implementation draft. The completion matrix records executable gates and
remaining acceptance gaps. A scalarized proof result is proof data only; it
does not grant source mutation, execution, law publication, database atomicity,
or removal of runtime guards.

`assurance_manifest::structured_law` accepts a checked same-module Program and
one function with no type parameters, effects, yields, or non-value parameters.
The type inventory admits immutable records with one to eight fields and
closed variants with one to eight cases. Scalar leaves are `i64`, `i32`, `u8`,
`usize`, and `bool`; aggregate nesting is at most three, and a flattened input
has at most 32 scalar leaves. Generic, recursive, mutable, aliased, resource,
class, collection, and unknown structures refuse before proof translation.
The compiler remains responsible for validating the source Program before
this lowering is called.

Every record field and variant case is selected through its persistent
declaration identity. The lowered scalar parameter inventory carries each
path of field IDs; source names only locate an already checked declaration.
The variant tag is a bounded `u8` with a staged domain precondition that
admits exactly the declared cases. Variant payloads are separate scalar
leaves; inactive payloads are unconstrained except for their scalar ranges.
An explicit value match admits exactly one unguarded arm for each case and
no wildcard. Arms may bind case fields and are lowered in source order to
conditional scalar expressions. The compiler's exhaustiveness check is a
prerequisite, not a proof of a branch's postcondition. Adding a case or
omitting an arm refuses the whole match.

Record and variant constructors evaluate fields left to right. An aggregate
result is substituted into every source postcondition, including projection
and explicit matches. The lowering makes every arithmetic operation in the
body a checked side obligation, even when its value is not used in the final
postcondition. Conditions, lazy boolean right operands, and selected match
arms guard those obligations according to their execution paths. Division,
remainder, calls, mutation, iteration, guarded match arms, and non-value
matches refuse. The existing typed scalar VC then owns binding, operation
order, checked integer ranges, and path-sensitive discharge.

SMT and Lean coverage are distinct. Z3 can discharge scalarized aggregate
result clauses and conditional match paths under the bounded `QF_LIA`
profile. Lean's additive structured profile admits scalar-result record and
variant projections with conditional expressions; the pinned recipe is
`simp_all <;> omega` when any theorem binder or goal contains an `if`.
The existing Lean v1 export retains its original profile, source bytes, and
golden gate. Bool-valued results, aggregate-result clause queries, and other
untranslated shapes remain outside the Lean structured profile. No report
may merge a Z3 success with an unsupported Lean shape into common support.

The source-bound certificate, independent replay, LAW-04 selected Project and
managed-Workspace attachment, and exact backend coverage report are still
open. Direct scalarized proof attempts cannot be promoted to a selected law
without those bindings. The reference model corpus must prove a bounded
two-account transfer's exact debit, credit, conservation, sufficient-balance
and amount-domain clauses, reject duplicate debit, wrong credit, arithmetic
trap, nonconservation and wrong failure case, and run the pinned Z3 and Lean
executables on positive and seeded negative examples before this row is
marked implemented.
