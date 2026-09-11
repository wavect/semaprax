# Public Generic Descriptor v1

Audience: compiler contributors producing descriptors, and authors of independent verifiers and foreign-language consumers.

Status: frozen wire-format specification with a reference codec, local
evidence, and a real-HIR producer (`src/public_generic_abi/descriptor.rs` and
its `producer` submodule). This is the descriptor half of gate #150-#152 of
the [Public Generic Ownership
milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md) and answers issues #170
and #151. The reference codec encodes and decodes a `DescriptorV1` value and
replays it byte-for-byte. `producer::generate_public_generic_descriptor`
(issue #151) derives that value from a real checked `ResolvedProgram` and a
caller-supplied source revision — see [Derivation from checked
facts](#derivation-from-checked-facts-the-producer) below. It does **not**
run [Public Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s
own classifier, which is issue #150's implementation half and does not exist
anywhere in this repository yet; the producer performs the v1 export-shape
predicate itself instead, with its own diagnostics, documented below. Public
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

## Derivation from checked facts (the producer)

`src/public_generic_abi/descriptor/producer.rs` exposes
`generate_public_generic_descriptor(program: &ResolvedProgram, source_revision: &str, export_id: &str) -> Result<GeneratedDescriptor, Diagnostic>`
(issue #151). Given a real checked `ResolvedProgram` (compiled through
`crate::parse` and `crate::hir::resolve`, never a hand-built fixture) and a
persistent export identity, it:

1. selects the export with `public_generic_surface::CandidateSurface::derive`
   (PG-3), which already refuses an unknown or ambiguous selection and a
   generic function template, and computes the complete substituted record
   closure reachable from the signature;
2. locally checks the v1 export-shape predicate — exactly one owned (`own`)
   input parameter, exactly one owned aggregate result, both fully concrete
   record instances, no declared effect — refusing with one of five new
   diagnostics (`SPX-PG705`-`SPX-PG709` below) otherwise, since [Public
   Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s own
   classifier (issue #150's implementation half) does not exist in this
   repository yet;
3. binds `program_root_digest` to a domain-separated digest over the sorted,
   deduplicated set of every persistent declaration identity in `program`
   (every checked type, function, function template, and function instance —
   never a display name, so a rename never moves it), `source_projection_digest`
   to a domain-separated digest of the caller-supplied `source_revision`, and
   `public_surface_digest` to the candidate surface's own digest, reused
   unchanged;
4. derives `settlement::plan` (PG-7) for the owned input, which already fails
   closed on any disagreement with the compiler's own cleanup inventory or
   cleanup plan, and exposes a `cleanup_inventory_digest`, `cleanup_plan_digest`,
   and `settlement_obligations_digest` on the returned `GeneratedDescriptor` —
   **not** folded into the eleven-field `DescriptorV1` wire preimage above,
   which this round does not widen;
5. renders the wire bytes, checks them against
   [`MAX_DESCRIPTOR_WIRE_BYTES`](#bounds), and self-verifies by independently
   replaying its own freshly encoded bytes before returning.

`program_root_digest`'s declaration-identity inventory is a deliberately
minimal real binding: it is rename-invariant and re-derived from checked
facts, but two programs sharing an identical declaration-identity set with
different field types or bodies are not distinguished by it alone. A future
round can widen it to a full structural digest using the same `TypeInventory`
the producer already builds; this round's determinism, rename-invariance, and
cross-pair replay requirements are met by the input/result instance digests
and the settlement digests already bound.

New diagnostics, in the descriptor's own `SPX-PG7xx` range (never the
classifier's reserved `SPX-PG6xx`):

| Code | Meaning |
| --- | --- |
| `SPX-PG705` | the selected export does not have exactly one owned aggregate input parameter |
| `SPX-PG706` | the export's one parameter is not owned (`own`) |
| `SPX-PG707` | the input or result position is not a fully concrete authored record instance |
| `SPX-PG708` | the export declares one or more effects |
| `SPX-PG709` | the rendered descriptor exceeds [`MAX_DESCRIPTOR_WIRE_BYTES`](#bounds) |

An unknown export, an ambiguous selection, or a selected generic template are
refused with `public_generic_surface`'s own `SPX-PG201` (reused, not
duplicated); a cleanup or settlement disagreement is refused with
`public_generic_settlement`'s own `SPX-PG501`/`SPX-PG502` (reused, not
duplicated).

## Verification and trusted replay (the independent verifier)

`src/public_generic_abi/descriptor/verify.rs` exposes
`verify_public_generic_descriptor(program, source_revision, expected_export_id,
expected_program_root_digest, candidate_bytes, options) ->
Result<VerifiedPublicGenericDescriptor, Diagnostic>` (issue #152). This closes
the descriptor trust boundary: **a submitted descriptor can name what must be
checked, but it can never supply the trusted facts used to validate itself.**
`program`, `source_revision`, `expected_export_id`, and
`expected_program_root_digest` are all supplied by the caller out of band;
none of the four is ever read back from `candidate_bytes` and treated as
authoritative.

This is not [the producer](#derivation-from-checked-facts-the-producer) run
backwards. Calling `generate_public_generic_descriptor` a second time and
byte-diffing its output against the candidate would only prove the producer
is deterministic against itself — already covered by its own 17 tests — and
would silently reproduce any bug in the producer's own final assembly step,
since both the "trusted" comparison value and the candidate check would then
come from identical code. Instead, the verifier independently reconstructs
the wire value from the same lower-level, already-tested primitives the
producer is built from (`CandidateSurface::derive`, and this module's own
re-derivation of the `program_root_digest`/`source_projection_digest`
algorithms this document already specifies, matching the specification
rather than importing the producer's private helpers) and assembles its own
`DescriptorV1` with the codec's own public `DescriptorV1::new`. The real
generator is still invoked once, on the same trusted facts, both because the
v1 shape-admission predicate is legitimately producer-owned logic that must
not be duplicated, and because the specification requires it be invoked; its
output is cross-checked against the independent reconstruction (a
disagreement is a producer-side defect signal, refused as `SPX-PG713`, never
an attacker signal) rather than trusted as the sole basis of acceptance.

### Verification phases

1. **Bound before parsing.** `candidate_bytes.len()` is checked against
   `options.max_descriptor_bytes` (clamped down to, never widened past,
   `MAX_DESCRIPTOR_WIRE_BYTES`) before any byte is interpreted.
2. **Strict structural parse.** `descriptor::decode` — the frozen codec's own
   parse — rejects malformed framing, an unknown schema literal, an oversized
   field, and trailing bytes. Reused, not reimplemented.
3. **Caller-independent trusted subject selection**, cheapest checks first:
   the candidate's own claimed export identity must equal
   `expected_export_id`; the caller's own `expected_program_root_digest` must
   equal the independently recomputed root of the `program` it supplied
   (catches a caller that passed a programme disagreeing with its own stated
   expectation); the candidate's embedded programme-root digest must equal
   that same recomputed root (the cross-pair defense) — checked once the
   trusted reconstruction below has run, since reading it requires this
   module's descendant-module access to `DescriptorV1`'s private field rather
   than a new public accessor that would widen the frozen codec's surface.
4. **Trusted reconstruction.** The real generator runs on
   `program`/`source_revision`/`expected_export_id` only; this module
   separately, independently reconstructs the same wire value and requires
   the two to agree.
5. **Exact canonical bytes.** `descriptor::replay` requires the candidate's
   identity preimage to equal the independently reconstructed value's,
   byte-for-byte — not merely digest-equal, and not only the top-level
   `identity_digest()`.

On success this returns a `VerifiedPublicGenericDescriptor`: a type with only
private fields and no public constructor other than
`verify_public_generic_descriptor` itself, and deliberately no
`From<ParsedPublicGenericDescriptor> for VerifiedPublicGenericDescriptor>`
anywhere in the module — naming an export in untrusted bytes is never
authority to adopt it. `ParsedPublicGenericDescriptor` (from the companion
`parse_public_generic_descriptor_selectors`) is the structurally distinct,
deliberately narrower type for the optional two-phase "look up a trusted
subject by descriptor" workflow: it exposes only the schema literal and the
*claimed* export id, never instance facts, digests, or a settlement plan.

### New diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-PG710` | the candidate's claimed export identity does not match the caller's independently supplied expected export |
| `SPX-PG711` | the caller's own expected programme-root digest does not match the independently recomputed root of the programme it supplied |
| `SPX-PG712` | the candidate's embedded programme-root digest does not match the trusted programme's independently recomputed root (cross-paired descriptor) |
| `SPX-PG713` | the real generator's output disagrees with this module's independent reconstruction of the same trusted facts (a producer-side defect signal) |

Every other refusal reuses an existing diagnostic exactly: `SPX-PG701`/`702`
from `decode`, `SPX-PG703`/`704` from `replay`, `SPX-PG705`-`709` from the
producer's own shape predicate, and `SPX-PG201` from
`CandidateSurface::derive` for an unknown, ambiguous, or generic-template
selection.

### Deterministic refusal precedence

Fixed, and pinned by the module's phase-specific tests: byte bound, then
strict structural parse, then the caller-independent export/root checks
(cheapest first), then trusted reconstruction and its shape-admission
refusals, then the generator-agreement cross-check, then the cross-paired
programme-root check, then the final exact-byte replay comparison. A case
that would pass an earlier phase is never used alone to exercise a later one.

### Recovery and currentness

This layer has no retained-store access of its own: `program` and
`source_revision` are supplied by the caller exactly as
[the producer](#derivation-from-checked-facts-the-producer) already requires,
and this module cannot itself distinguish "the caller's current head" from
"a deliberately selected historical revision" without one. `VerificationOptions::historical_mode`
is a caller-declared intent flag, recorded on the returned
`VerifiedPublicGenericDescriptor` for downstream audit; it does not relax any
check. Currentness policy — whether a given `program`/`source_revision` pair
is the caller's current head — is entirely the caller's own retained-store
responsibility, matching "preserve current architecture" for this round: no
`ProgramRoot`, Project candidate, or workspace-session type is threaded
through this layer yet.

### Nonclaims specific to verification

The verifier decides nothing about which exports are admitted under a
general classifier (still [Public Generic Boundary Profile
v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s unimplemented job); it reuses
the producer's own local shape substitute exactly as the producer does. It
performs no build, filesystem write, registry access, network call, or code
execution, and a `VerifiedPublicGenericDescriptor` is not a public ABI and
must not be cited as a support or publication claim. It does not implement
provider execution or a language-specific consumer — that is later issues'
work.

## Nonclaims

This descriptor exposes no internal HIR layout, no C struct, no Rust
monomorphization detail, and no Wasm memory offset. It reuses no Project
v9/v11 descriptor bytes and widens none of them. It grants no execution,
build, filesystem, registry, or publication authority. The reference codec's
own `tests` submodule still uses hand-constructed `DescriptorV1` fixtures for
wire-format determinism and hostile-input evidence; the producer's own tests
derive every value from a real compiled program instead, but a
`GeneratedDescriptor` still names no shipped ABI and must not be cited as a
support or publication claim. Neither the codec nor the producer decides
which exports are admitted under a general classifier — that is [Public
Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s job,
whose own classifier (issue #150's implementation half) does not exist yet;
the producer's export-shape checks are a local, descriptor-scoped
substitute, not that classifier, and should be revisited once it lands.
