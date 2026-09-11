# Public Generic Descriptor v1

Status: frozen wire-format specification with a reference codec and local
evidence (`src/public_generic_abi/descriptor.rs`). This is the descriptor half
of gate #150-#152 of the [Public Generic Ownership
milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md) and answers issue #170.
The reference codec encodes and decodes a `DescriptorV1` value and replays it
byte-for-byte; it does **not** derive that value from real checked HIR,
`ProgramRoot`, or a Project candidate — that derivation is explicitly out of
scope for this round (scope reconciliation and contract freeze only, no
compiler/backend implementation) and is the next tranche's work. Public
generic ownership remains unsupported and unpublished.

Audience: ABI, package, evidence, and generated-consumer maintainers.

## Scope

A `DescriptorV1` names one exported function admitted by [Public Generic
Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md): its persistent
export identity, its input and result generic instances (reusing [Public
Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md)'s own
`InstanceFacts` rather than re-deriving a parallel type-fact structure — see
[Reuse](#reuse-rather-than-reinvention)), and the exact revision/candidate
context it is bound to. It is read-only, target-neutral, and grants no
authority: generating a descriptor performs no build, filesystem write,
registry access, or code execution.

| Layer | Identifier |
| --- | --- |
| Descriptor schema | `semaprax.public-generic-descriptor.v1` |
| Identity digest domain | `semaprax.public-generic-descriptor.v1.identity\0` |
| Depends on | [Public Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) |
| Depends on | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) |
| Reserved/allocated diagnostic range | `SPX-PG7xx` |

`rg -n "public-generic-descriptor" docs src tests` at freeze time (commit
`d45db653`) found no colliding schema; this is a new artifact, never a
reinterpretation of the Canonical ABI Report or Project v8/v9/v11 descriptor
bytes.

## Reuse rather than reinvention

Per the repository's "do not create a parallel framework when an owning
helper already exists" rule, a descriptor's input and result facts are the
existing `public_generic_type::InstanceFacts` value unchanged: its `term`
(the canonical grammar spelling), `template` identity, ordered `arguments`,
substituted `fields`, `owned_leaves`, and `instance_digest`. The descriptor
does not re-derive template or argument identity; it binds to the digest the
grammar already computes.

## What the descriptor binds

A descriptor is meaningless without the context it was derived in. Binding
every one of the fields below is what keeps a stale or cross-paired
descriptor from being replayed against a different program:

- **Export identity** (`export_id`): the persistent `@id` of the selected
  function, never its display name.
- **Boundary-profile identity** (`boundary_profile`): the exact
  `semaprax.public-generic-boundary-profile.v1` string, so a future v2
  profile cannot be silently accepted by a v1-only consumer.
- **Type-grammar identity** (`type_grammar_schema`): the exact
  `semaprax.public-generic-type-grammar.v1` string the input/result terms
  were rendered against.
- **Program-root digest** (`program_root_digest`): an opaque, already-computed
  binding digest naming the exact checked program the export was classified
  in. The reference codec treats this as an opaque framed byte string; *how*
  it is computed from a real `ProgramRoot` is the next tranche's work, not
  this one's.
- **Source-projection digest** (`source_projection_digest`): likewise opaque
  here, binding the exact source projection (revision) the classification
  used.
- **Public-surface digest** (`public_surface_digest`): likewise opaque,
  binding the exact candidate/public-surface delta context (see [Public
  Generic Candidate Delta v1](PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md)) the
  export was selected from.
- **Input instance facts** and **result instance facts**: the grammar's own
  `term` and `instance_digest`, one pair per position.

Any later phase that finds these bindings inconsistent with its own trusted
context must reject the descriptor rather than repair or ignore the
mismatch — this is what "independent replay" means in the sections below.

## Canonical bytes

A descriptor's **identity preimage** is the length-framed concatenation, in
this exact field order, of:

```text
frame(schema)
frame(boundary_profile)
frame(type_grammar_schema)
frame(export_id)
frame(program_root_digest)
frame(source_projection_digest)
frame(public_surface_digest)
frame(input.term)
frame(input.instance_digest)
frame(result.term)
frame(result.instance_digest)
```

where `frame(bytes)` is an 8-byte little-endian length prefix followed by the
bytes themselves — the same framing convention already used by
`public_generic_type` and `public_generic_settlement`. The descriptor's
**identity digest** is the domain-separated SHA-256 of that preimage, using
the same `sha256:<hex>` rendering and the same length-then-domain-then-bytes
digest convention those modules use.

The **wire encoding** (`encode`) is the identity preimage followed by one more
framed field, `frame(export_name)`, carrying the display name as
presentation. This is the only field excluded from the identity preimage and
therefore from the identity digest: **a display-name rename changes the wire
bytes but never the identity digest**, matching the milestone's "no nominal
shortcuts" invariant and issue #170's acceptance criterion that a rename must
preserve identity.

No field is optional and no field may be empty except `export_name`
(presentation) and the two digest fields' *content*, which are opaque bytes
whose emptiness is a decode-time capacity/format question, not a schema
question.

## Decoding and independent replay

`decode` parses the wire bytes strictly: it requires the exact field order
above, requires every length prefix to describe bytes actually present (no
truncation, no trailing bytes, no oversized claim), and recomputes the
identity digest from the parsed fields rather than trusting a transmitted
digest — there is no transmitted digest field; the digest is always derived,
never carried, so it can never be forged independently of the bytes it
covers.

`decode` alone does **not** validate that the descriptor matches any
particular trusted program; it only validates that the bytes are a
well-formed `DescriptorV1`. Binding validation is `replay`'s job:

`replay(candidate: &[u8], trusted: &DescriptorV1) -> Result<DescriptorV1, Diagnostic>`
decodes `candidate`, then requires its identity preimage to equal the
trusted value's identity preimage byte-for-byte (not merely digest-equal,
matching `public_generic_type::verify_term`'s convention of comparing
recomputed bytes rather than trusting a submitted digest). The trusted value
in this reference codec is a plain `DescriptorV1` the caller already
possesses; the next tranche's classifier is what will produce a trusted value
from real checked HIR instead of a test fixture.

## Failure and security cases

- **Forged digest, correct-looking bytes.** Impossible by construction: there
  is no transmitted digest to forge. `replay` compares preimage bytes.
- **Reordered or duplicated fields.** Rejected at `decode`: field order is
  fixed and the total consumed length must equal the input length exactly.
- **Truncated or oversized length prefix.** Rejected at `decode` before any
  field is interpreted.
- **Cross-paired descriptor** (correct format, wrong program/candidate).
  Rejected at `replay`: the `program_root_digest`, `source_projection_digest`,
  or `public_surface_digest` fields differ from the trusted value's, so the
  identity-preimage comparison fails.
- **Stale schema or boundary-profile version.** Rejected at `decode` if the
  `schema` field is not exactly `semaprax.public-generic-descriptor.v1`;
  rejected at `replay` if `boundary_profile` or `type_grammar_schema` differ
  from the trusted value's, even when the bytes are otherwise well-formed.
- **A display rename.** Changes `encode`'s output bytes (the trailing
  `export_name` field) but not the identity preimage or identity digest;
  `replay` against the old trusted value still succeeds if only the name
  changed, because `replay` compares the identity preimage, not the full wire
  bytes.
- **Silent partial admission.** Not possible: every reachable field of the
  underlying `InstanceFacts` is already a closed grammar term; the descriptor
  carries `term` and `instance_digest` as one unit and never a subset of
  fields.

## Bounds

Reused from [Public Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#bounds):

| Bound | Value |
| --- | --- |
| Max bytes per instance term (`input.term`, `result.term`) | 65,536 |
| Max total wire bytes per descriptor | 131,072 |

`decode` and `replay` both reject before allocating past these bounds; a
descriptor that would exceed either is `SPX-PG702`, never truncated.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-PG701` | malformed descriptor bytes: bad framing, wrong schema literal, trailing bytes, or truncated field |
| `SPX-PG702` | a descriptor bound was reached (total bytes or a framed field's length) |
| `SPX-PG703` | independent replay found the recomputed identity preimage does not equal the submitted one |
| `SPX-PG704` | the descriptor's `boundary_profile` or `type_grammar_schema` does not match the trusted value's, even though the bytes otherwise decode |

`SPX-PG7xx` is the range this document allocates; `SPX-PG6xx` stays reserved
for the boundary-profile classifier (not implemented this round) and
`SPX-PG8xx` is allocated to [Public Generic Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md)
below it.

## Evidence

Local evidence only, in `src/public_generic_abi/descriptor.rs` and its
`tests` submodule: golden byte-determinism (`encode` of the same value is
byte-identical across calls and independent of construction order), a
display-rename case that changes wire bytes but not the identity digest, and
hostile decode/replay cases covering truncation, trailing bytes, reordered
framing, an unknown schema literal, an oversized length claim, and a
cross-paired trusted value for every one of the seven bound fields. No hosted
run is recorded for this document; see the accompanying worktree report for
the exact local commands run.

## Nonclaims

This descriptor exposes no internal HIR layout, no C struct, no Rust
monomorphization detail, and no Wasm memory offset. It reuses no Project
v9/v11 descriptor bytes and widens none of them. It grants no execution,
build, filesystem, registry, or publication authority. It is not derived from
real checked HIR in this round; the `DescriptorV1` values in its tests are
hand-constructed fixtures, not compiler output, and must not be cited as
evidence that a real export has been classified or described. It does not
itself decide which exports are admitted — that is [Public Generic Boundary
Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s job, whose classifier
does not exist yet either.
