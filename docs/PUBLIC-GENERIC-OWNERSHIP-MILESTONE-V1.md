# Public Generic Ownership Milestone v1

Status: open milestone, separately gated. No public generic ownership surface is
admitted, generated, published, or supported at this commit. This document owns
the milestone's identity, its nine prerequisite gates, the separation
invariants that keep it independent of internal generic work, and the standing
support and publication decision. It is a charter, not evidence.

Audience: language, ABI, package, evidence, and promotion reviewers.

## Why this is a separate milestone

Public generic ownership is the surface on which a *foreign* consumer observes
a generic template, its ordered concrete arguments, the substituted field tree
of an owned value, and that value's allocation and failure settlement across a
target boundary. Nothing in that list is a consequence of admitting a generic
inside a SEMAPRAX body.

[Concrete Generic Owned-Byte Records v1](CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md)
and the GEN-06 contracts closed bounded *internal* generic semantics: exact
instance identities, ordered substitutions, ownership and call facts, cleanup
replay, and backend execution. Every observable parameter and result in those
profiles stays scalar or a borrowed byte slice; the template, the concrete
instance, the fields, the layout, and the owner remain internal to the body.
Closing more of that work produces no public generic signature, descriptor,
carrier, consumer, or support claim, and the reverse also holds: this milestone
may not be advanced by widening an internal admission profile.

Treating the public surface as a side effect of GEN-05 would fail four ways at
once. It would publish an unversioned type spelling as an interchange format;
it would let a display rename or an argument reordering change a foreign
calling convention silently; it would hand a consumer an owned allocation with
no stated settlement on failure; and it would convert a single green internal
Linux job into a cross-platform support claim. The milestone exists so each of
those becomes its own artifact with its own executable gate.

## Milestone identity

| Layer | Identifier |
| --- | --- |
| Milestone | `semaprax.public-generic-ownership.v1` |
| Roadmap workstream | ABI-09, [public generic programme](ROADMAP.md) |
| Prerequisite gates | `PG-1` through `PG-9` |
| Separation gate | `public_generic_ownership_milestone` in the projections harness |

The milestone identifier names the programme. It is deliberately not a Project
schema, profile, descriptor, carrier, or prelude version: a public generic
surface requires new versioned artifacts of its own rather than reinterpreted
Project v8, v9, or v11 bytes.

## Prerequisite gates

Each gate is independent, has one owning artifact, and is advanced only by that
artifact's executable evidence. The state column uses exactly three values:

- `Open` — nothing is implemented for it.
- `Implemented, local evidence` — the code and its executable gate exist and
  pass locally. No hosted run is recorded, so this is not hosted evidence and
  not a support claim.
- `Hosted green` — a hosted run and job are recorded in the owning artifact for
  an exact implementation commit.

PG-8 is what converts local evidence into hosted evidence; no other gate may
record `Hosted green` before it does. PG-9 may only move once every other gate
reads `Hosted green`.

| Gate | Requirement | Owning artifact | State |
| --- | --- | --- | --- |
| PG-1 | A new versioned target-neutral type grammar: closed vocabulary, injective canonical term, byte-exact render and parse, domain-separated digest, and fail-closed rejection of every construct outside the admitted surface | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) | Implemented, local evidence |
| PG-2 | Explicit template identity and ordered argument identities: persistent template declaration identity, declared arity, positional parameter owner and index, and digests that distinguish permutation, omission, duplication, and substitution | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) | Implemented, local evidence |
| PG-3 | Semantic compatibility rules: a closed classification over two grammar surfaces with explicit reasons, no compatibility inferred from a diff classification, and no version decision inferred from a classification | Pending its owning specification | Open |
| PG-4 | Candidate ABI-delta evidence that selects the public generic signature, retains ordered arguments and substituted fields, and survives mutation, recovery, and independent byte-exact replay | Extension of [Candidate ABI Delta v1](PROJECT-CANDIDATE-ABI-DELTA-V1.md) | Open |
| PG-5 | Generated Rust, TypeScript/Wasm, C, and C++ consumers derived from the grammar, byte-deterministic, with no ambient authority | Pending its owning specification | Open |
| PG-6 | Hostile metadata replay: forged, stale, truncated, reordered, and mutated grammar or descriptor bytes fail closed in every consumer route and in independent replay | Pending its owning specification | Open |
| PG-7 | Owned allocation and failure settlement across the boundary: bounded allocation, exact copy-out, sticky failure selection, canonical cleanup order, and equal checked behavior on interpreter, native C11, and Core Wasm | Pending its owning specification | Open |
| PG-8 | Cross-platform hosted evidence for the complete milestone corpus on Linux, macOS, and Windows, recorded for an exact implementation commit | Pending its owning specification | Open |
| PG-9 | An explicit support and publication decision naming the exact version, target, and consumer scope, with its prerequisite profile decisions | This milestone | Open |

PG-1 through PG-8 are prerequisites of PG-9, not substitutes for it. A complete
set of green prerequisite gates authorizes the decision to be *made*; it does
not make it.

## Separation invariants

These hold for every change to internal generic semantics, including changes
that are otherwise fully gated and hosted green.

- An internal generic admission never admits a public generic signature. The
  template, concrete instance, fields, layout, and owner stay inside the body,
  and every public export keeps its already-admitted signature.
- No existing descriptor, carrier, package, prelude, graph, or cleanup schema is
  reinterpreted to carry a generic surface. A public generic surface requires a
  new versioned descriptor and carrier.
- A green internal gate is never evidence for a PG gate, and a green PG gate is
  never evidence for another PG gate.
- Neither a CI label, a generated package, a registration, nor an archive is a
  support or publication decision. Only PG-9 is.
- The public projections named below must keep rejecting a generic signature
  with their exact recorded reasons until the owning PG gates are green. The
  milestone's separation gate asserts this executably, so an accidental
  widening reddens the build instead of becoming a silent public claim.

## Recorded public-boundary rejections

These are the exact facts the separation gate pins. They are rejections by the
current public projections, not a list of intended future spellings.

| Public projection | Selected generic surface | Recorded outcome |
| --- | --- | --- |
| [Canonical ABI report](ABI-REPORT-V1.md) | a generic function | excluded, reason `generic_function` |
| [Canonical ABI report](ABI-REPORT-V1.md) | a monomorphic function returning a concrete generic instance | excluded, reason `unsupported_result_type` |
| [C header emission](C-HEADER-V1.md) | the same two selections | excluded with the same closed reasons |
| Project v9 and v11 public API descriptors | a selected generic result | rejected before descriptor or target generation |
| [Wasm scalar exports](WASM-SCALAR-EXPORTS-V1.md) | a generic or generic-returning export | rejected with `SPX-W115` |

An exclusion is a closed report reason, never a partial admission. A rejection
is a diagnostic, never a backend accident.

## Standing support and publication decision

As of 2026-09-11: **public generic ownership is not supported and not
published.** No stable C, Rust, WIT, Component, npm, Project, or package
representation of a generic template, instance, or owned generic value exists,
and none may be described as supported, hosted, or production-ready.

The decision is revisited only when all of the following hold together:

1. PG-1 through PG-8 are green for one exact implementation commit, each
   recorded in its owning artifact with its hosted run and job.
2. The named scope is explicit: which grammar version, which descriptor and
   carrier version, which consumer languages, which targets, and which
   platforms.
3. The prerequisite profile decisions it depends on are themselves decided
   rather than assumed.
4. Fresh evidence is required for later code changes instead of attributing
   them to this milestone's commit.

Partial completion changes nothing. Eight green gates and an unmade decision
still mean unsupported and unpublished.

## Nonclaims

This document admits no syntax, defines no descriptor, carrier, package, or
calling convention, generates nothing, executes nothing, and grants no source,
filesystem, process, network, execution, signing, publication, or registration
authority. It does not promote, deprecate, or reinterpret any existing profile,
and it is not evidence that any gate has passed. The
[completion matrix](COMPLETION-MATRIX.md) owns product status; the
[quality gates](QUALITY-GATES.md) own required verification.
