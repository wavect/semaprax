# Finite Structured Law v1 (LAW-07)

Status: implemented bounded proof profile. The completion matrix records the
executable gates and explicit limits. A scalarized proof result is proof data only; it
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

## Source-bound installed Project proof

`prove_structured_postcondition` selects one stable declaration and
postcondition in a retained, fully admitted Project. It reparses the exact
retained source and admits only declarations present in selected Project HIR.
A scalarized result alone has no authority. The installed capability checks a
satisfiable precondition domain and the generated SMT query, or a bounded
checked domain witness and every generated Lean theorem. Failures yield no
opaque `VerifiedProjectProof`.

The canonical `semaprax.structured-law-project-certificate.v1` record binds
the compiler version, Project revision, ProgramRoot, source path/revision/
digest, selected postcondition, lowered scalar revision, ordered scalar leaf
inventory with persistent field paths, dependent declaration IDs, exact
query digest, backend, installed tool version, translation profile, domain
witness method, backend coverage, and trust/nonclaim inventory. The JSON is
inert. `replay_structured_postcondition` rederives every field and query fact
from the retained Project and reruns the installed backend before creating a
new opaque proof. Reordered fields or an added variant case therefore require
reproof; they cannot retarget an old certificate. Formatting-only source
changes also invalidate this exact-byte certificate conservatively even when
persistent declaration IDs and field paths remain stable.

The Project assurance route joins the opaque proof only to a matching
postcondition, source row, Project revision, and ProgramRoot. Strict law
policy uses distinct `PinnedStructuredSmtSource` and
`PinnedStructuredLeanSource` translation requirements; ordinary scalar and
modular method profiles cannot satisfy them. The method record and
certificate state exact backend coverage independently. Lean currently
supports scalar-result record/variant laws and declines aggregate-result
clauses; Z3 supports those clause queries. Neither backend's status is
silently merged into a common proof claim.

The trusted base includes the local installed solver/kernel, the compiler's
source-to-scalar lowering and its unverified translation to SMT/Lean, the
retained Project authentication path, and checked arithmetic/range modeling.
These proofs do not establish native/Wasm lowering preservation, physical
database transaction atomicity, execution, source mutation, or publication.

Focused installed Project replay, a 196-state two-account reference corpus,
and recursive/mutable/borrowed refusals have passed locally. A selected
four-module Project and managed Workspace now execute and physically publish
a private scalar-signature law whose body constructs and projects records.
The source-bound structured proof must be refreshed after a candidate change;
stale and missing proofs refuse before staging, while an accepted proposal
alone pivots `ACTIVE`. Reordered fields and newly added variant cases cannot
reuse the old certificate.

This gate keeps the required web export in a disconnected scalar module. The
Public Scalar Export Profile v1 still refuses authored record/variant types in
the selected public export closure (`SPX-W115`); no aggregate public ABI is
claimed. Direct checked-source Z3 proves aggregate-result transfer clauses,
while installed Project attachment is currently bounded to scalar-signature
functions with private aggregate bodies. Lean proves its separately named
scalar-result structured subset and reports aggregate-result clauses as
unsupported. All other listed limits continue to refuse rather than confer
an enclosing proved status.
