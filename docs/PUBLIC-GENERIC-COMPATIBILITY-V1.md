# Public Generic Compatibility v1

Status: implemented bounded projection, hosted green on Linux, macOS, and
Windows; gate PG-3 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
A candidate surface is a description, not an admission: no public generic
signature is admitted by describing one, and public generic ownership remains
unsupported and unpublished.

Audience: ABI, package, evidence, and promotion reviewers.

## Scope

Two artifacts, both read-only projections of already-checked HIR:

| Layer | Identifier |
| --- | --- |
| Candidate surface | `semaprax.public-generic-candidate-surface.v1` |
| Compatibility comparison | `semaprax.public-generic-compatibility.v1` |
| Type spelling | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) |

A **candidate surface** describes selected exports of one checked program: for
each, its declared effects, its ordered parameter positions with ownership
modes, its result, and every record instance reachable from those signatures.
A **comparison** classifies one ordered pair of surfaces.

Selection is by persistent declaration identity, never by display name.
Between 1 and 64 unique identities are accepted. An unknown identity, a
repeated one, a type declaration, and a generic template all fail closed with
`SPX-PG201`: a public surface names no type parameters, so describing one
instantiation of a template would be an invention rather than a description.

## Signature positions

The grammar spells data types. A signature also has positions that are not
data types, so the surface adds its own closed position vocabulary rather than
widening the grammar:

| Kind | Spelling | Source type |
| --- | --- | --- |
| `data` | a grammar term | any type the grammar admits |
| `borrowed_byte_view` | `view:slice-u8` | `Slice<u8>` |
| `borrowed_text_view` | `view:str` | `str` |

The `view:` prefix is unreachable for a grammar term, so the two vocabularies
cannot collide. Any other position is a grammar rejection (`SPX-PG101` with its
closed reason), not a silently described surface.

## Compatibility rules

The verdict is one of `unchanged`, `compatible`, or `breaking`, and it is the
heaviest finding. Every difference produces at least one finding, and every
finding carries one closed reason bound to the subject it was found on.

| Reason | Verdict | Subject |
| --- | --- | --- |
| `export_added` | compatible | the export identity |
| `export_removed` | breaking | the export identity |
| `effects_changed` | breaking | the export identity |
| `parameter_count_changed` | breaking | the export identity |
| `parameter_ownership_changed` | breaking | `<export>#<index>` |
| `parameter_type_changed` | breaking | `<export>#<index>` |
| `result_type_changed` | breaking | the export identity |
| `instance_template_changed` | breaking | the instance term |
| `instance_arguments_changed` | breaking | the instance term |
| `instance_fields_changed` | breaking | the instance term |
| `instance_owned_leaves_changed` | breaking | the instance term |
| `reachable_instance_added` | compatible | the instance term |
| `reachable_instance_removed` | compatible | the instance term |

Four rules carry the substance.

**Presentation is never compatibility.** Renaming an export, a parameter, a
record, a type parameter, or a field yields `unchanged` and leaves the surface
digest identical, because every identity-bearing fact is a persistent
identity, a declared arity, an ordered position, or an ownership mode. The
rendered surface still shows the new names; only the digest preimage excludes
them. A parameter's value identity is excluded for the same reason the
repository excludes it elsewhere: expression identities may be revision-scoped,
and a revision-scoped fact must not move a compatibility verdict.

**Ordered arguments are ordered.** A permuted type-argument vector is a
different instance, so the parameter term changes and the reachable closure
swaps one instance for another. Omission is not a permutation but an
`arity_mismatch` grammar rejection.

**A reachable field change is breaking, even where source compatibility is
not.** A foreign consumer reads the whole substituted field tree and the
owned-leaf shape of what it receives. Adding a Copy field to a nested record
changes no canonical term, no parameter position, and no owned-leaf path — and
is still breaking, reported once on the record that changed rather than on
every position that mentions it. Adding an owned field additionally changes the
owned-leaf shape of that record *and* of every instance reaching it, so both
reasons fire and neither hides the other.

**Reachability is informational.** A reachable instance appearing or
disappearing is `compatible`, because its cause is already classified at the
entry it became reachable from. It is recorded rather than dropped so a reader
can see the closure move.

Entry-level reasons are about the signature shape — the position kind and the
canonical term. What a term denotes is compared once, on the instance itself.

## Bounds, replay, and diagnostics

| Bound | Value |
| --- | --- |
| Selected exports | 64 |
| Reachable instances | 256 |
| Canonical bytes, either artifact | 1,048,576 |

Both artifacts are canonical compact JSON plus one trailing newline, with
byte-ordered keys. `CandidateSurface::verify` and `verify_comparison`
independently recompute their artifact and require byte equality; a missing
trailing newline is a mismatch, and the comparison is directional. Submitted
bytes are never treated as source, HIR, identity, or authority.

| Code | Meaning |
| --- | --- |
| `SPX-PG201` | the selection is empty, repeated, unknown, or not a candidate export |
| `SPX-PG202` | a surface bound was reached |
| `SPX-PG203` | submitted surface bytes are not the recomputed surface |
| `SPX-PG204` | submitted comparison bytes are not the recomputed comparison |

## Hosted evidence

Hosted evidence: the milestone corpus passed on `ubuntu-latest`, `macos-latest`,
and `windows-latest` for implementation commit `2ef043ba1b989f49b256e456f71fb6e89068bf33` in
[run 34594793245](https://github.com/wavect/semaprax/actions/runs/34594793245). That is evidence for the corpus this document owns, not for the
milestone's remaining gates.

## Nonclaims

A classification is not a semantic-version decision, and it is not a support or
publication decision. Nothing here maps a verdict onto a version bump or a
promotion, and both artifacts carry `semantic_version_decision: not_inferred`,
`support: not_assessed`, and `publication: not_assessed` in their own fields.
A surface observes no runtime, allocates nothing, settles no failure, emits no
consumer, and defines no descriptor, carrier, package, or calling convention.
Describing a candidate export does not admit it: the public projections still
reject generic signatures, and the milestone's separation gate continues to
prove it.
