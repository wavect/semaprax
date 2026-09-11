# Public Generic Candidate Delta v1

Status: implemented bounded candidate route with **local evidence only; no
hosted run is recorded** for it. It advances gate PG-4 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)
no further than `Implemented, local evidence`. A delta is a description of two
candidate surfaces: no public generic signature is admitted by describing one,
no public projection is widened, and **public generic ownership remains
unsupported and unpublished**.

Audience: ABI, package, evidence, and promotion reviewers, and agents reviewing
immutable Project candidates.

## Scope

`ProjectCandidate::public_generic_delta(expected_candidate)` emits a
deterministic, candidate-bound comparison of the public generic *surfaces* of
the exact immutable base and final candidate revisions. It is the direct
analogue of [Candidate ABI Delta v1](PROJECT-CANDIDATE-ABI-DELTA-V1.md) for the
versioned public generic artifacts rather than for the compiler's internal
identity keys.

| Layer | Identifier |
| --- | --- |
| Delta report | `semaprax.project-candidate-public-generic-delta.v1` |
| Verification record | `semaprax.project-candidate-public-generic-delta-verification.v1` |
| Type spelling | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) |
| Described surface | [Public Generic Compatibility v1](PUBLIC-GENERIC-COMPATIBILITY-V1.md) |
| Facts digest domain | `semaprax.candidate-public-generic-delta.facts.v1\0` |
| Report digest domain | `semaprax.candidate-public-generic-delta.report.v1\0` |

Both digest domains are new. The candidate-ABI-delta domains are not reused: a
different projection of the same revisions must not produce a digest another
route could be asked to accept.

The route is read-only. It requires the exact candidate digest before doing any
work, invokes no compiler, executes no target code, creates no file, reads
nothing back from a previously emitted artifact, and grants no authority.

## Selection basis

The selected export inventory is the manifest's complete `web_exports` set plus
its command function when present — the same basis as the candidate ABI delta,
and it is stated in the report's `selection_basis` field. Selection is by
stable declaration identity, never by display name, so a rename cannot add,
drop, or reorder a selection.

For each of the two revisions independently, the route builds a
`TypeInventory` from the retained projection modules' type declarations,
collects those modules' monomorphic functions and their generic template
identities, and describes the selected exports the grammar can spell. Nothing
is shared between the two sides except the selection rule itself.

## Exclusions

An export the grammar cannot spell is **not** a failure of this route. It is
recorded as an exclusion carrying the position it was refused at and the
refusing artifact's own closed reason:

| Position | Meaning |
| --- | --- |
| `parameter#<index>` | that parameter's type is outside the grammar |
| `result` | the result type is outside the grammar |
| `selection` | the identity is a generic template, is absent from the retained functions, or resolves to two of them |

Positions are examined left to right — parameters in order, then the result —
and the first refusal is the recorded one. The reason vocabulary is closed: the
grammar's twelve rejection reasons (`type_parameter`, `owned_string`,
`borrowed_str`, `borrowed_byte_view`, `unit`, `inline_byte_array`,
`function_type`, `compiler_owned_nominal`, `unadmitted_nominal_kind`,
`missing_declaration`, `ambiguous_declaration`, `arity_mismatch`) plus
`not_a_candidate_export` for a selection refusal and `grammar_bound` for a
grammar or surface bound. A reason is extracted from the owning artifact's own
diagnostic and validated against that artifact's enumeration, so a grammar that
grows a reason makes this route fail closed rather than emit an unrecognized
one.

This matters because of what exists today. Every export a Project manifest
admits is scalar or borrowed-view shaped, so at least one of its positions is
refused and **an all-excluded delta is the normal case**. Such a report is
complete and useful: it records the selection, the exact refusal, and that no
surface was described. Turning it into an error would make the route unusable
exactly where it has to be usable, and would hide the fact that no public
generic surface exists yet.

An included export is one whose every position the grammar spells. It need not
carry a record instance: a wholly scalar signature is a described surface with
no reachable instance.

## Comparison

The two described surfaces are compared with
[`public_generic_surface::compare`](PUBLIC-GENERIC-COMPATIBILITY-V1.md). The
report carries the verdict, every finding with its closed reason and its own
verdict weight, both surface digests, and the PG-3 comparison artifact whole.
The `comparison.basis` field names which of three states produced the verdict:

| Basis | When |
| --- | --- |
| `public_generic_compatibility_v1_over_both_described_surfaces` | both revisions describe a surface; the verdict is exactly the PG-3 comparison's |
| `no_described_export_on_either_revision` | neither describes one, so nothing described changed: `unchanged`, no finding, both surface digests `null` |
| `described_export_presence_only_one_revision_describes_a_surface` | exactly one describes one; the described export set itself moved, spelled with the PG-3 `export_added`/`export_removed` reasons. There is no second surface, so no other difference is classified |

Each side of the report also carries its described identities, its exclusions,
its surface digest, its reachable-instance count, and — when a surface exists —
that surface's canonical bytes as a value, including every template identity,
every ordered argument, every substituted field in declaration order, and every
transitive owned leaf.

Presentation is not compatibility. A display rename applied through a semantic
change leaves both surface digests and the verdict unchanged while the rendered
surfaces show the new name, and the authored evidence pins that on a real
`rename_declaration` transaction.

## Binding

The report binds, in its own fields: the expected candidate digest, the exact
base and final Project revisions, the exact base and final workspace revisions,
both semantic graph digests, and a domain-separated digest of the canonical
comparison facts. Every one of those is re-derived from the immutable
candidate, never read back from submitted bytes.

It also carries explicit, always-present status fields:
`compatibility_authority` (this is a description of candidate surfaces;
compatibility, support, and publication remain with the milestone), `admission`
(candidate description only; no public generic signature is admitted and no
public projection is widened), `support: not_assessed`,
`publication: not_assessed`, `runtime: not_observed`, and
`semantic_version_decision: not_inferred`, alongside false source, filesystem,
execution, publication, and deployment authority flags.

## Bounds and diagnostics

| Bound | Value |
| --- | --- |
| Canonical report bytes | 4,194,304 |
| Charged fact work bytes | 8,388,608 |
| Emitted facts | 4,096 |
| Visits | 65,536 |

The report is bounded well above the 1 MiB surface bound because it embeds up
to two surfaces and one comparison. The selected-export, reachable-instance,
and term bounds remain the owning artifacts' own, and are echoed in the
report's `limits` field. Reaching any bound is a refusal, never a truncated or
repaired report.

| Code | Meaning |
| --- | --- |
| `SPX-PG301` | retained candidate facts cannot be described by this route |
| `SPX-PG302` | a delta bound was reached |
| `SPX-PG303` | submitted bytes are not the independently recomputed report |

Existing candidate replay and stale-selector diagnostics remain authoritative
and fire first: a malformed selector is `SPX-G222` and a selector that is not
this candidate's is `SPX-G224`, both before any surface is described.

## Replay

`ProjectCandidate::verify_public_generic_delta(expected_candidate, bytes)`
bounds the submitted bytes, independently replays the complete candidate from
its retained base and typed history, recomputes the report, and requires byte
equality. It returns a separate verification record carrying the submitted
report's domain-separated digest and `submitted_bytes_authority: false`.

Submitted JSON is never treated as source, HIR, target evidence, a surface, a
verdict, or authority: it is compared, never read. Authored evidence in the
consolidated Project-candidate harness covers the exact bytes, a single-byte
mutation, a truncation, empty bytes, a JSON-equal re-serialization with
reordered keys, a tampered status field, the report of a different candidate of
the same base, an oversized submission, and a candidate restored from its
recovery capsule recomputing byte-identical bytes.

## Nonclaims

- Not a public generic signature admission. Describing a candidate export does
  not make its signature public, does not widen any language or Project
  profile, and does not change what the existing public projections accept —
  they still reject generic surfaces, and the milestone's separation gate
  continues to prove it.
- Not a semantic-version, support, or publication decision. No verdict maps
  onto a version bump or a promotion; only PG-9 decides support and
  publication, and it has not.
- Not runtime, allocation, settlement, or external-consumer evidence. The route
  observes no execution, allocates nothing across a boundary, and settles no
  failure.
- Not a descriptor, carrier, package, calling convention, layout, or memory
  representation.
- Not hosted evidence for any milestone gate. This artifact records local
  evidence for PG-4 only; PG-8 is what converts local evidence into hosted
  evidence, and a green PG gate is never evidence for another.
- No source, filesystem, process, network, execution, signing, publication, or
  deployment authority.

The [completion matrix](COMPLETION-MATRIX.md) owns product status and the
[quality gates](QUALITY-GATES.md) own required verification; this document
changes neither.
