# Public Generic Ownership Milestone v1

Status: open milestone, separately gated. Five of its nine prerequisite gates
are hosted green for one exact implementation commit; three more (PG-5, PG-6,
PG-7) now have real code and passing local gates but no hosted run and named
open gaps (see the gate table below). PG-9 remains undecided. No public
generic ownership surface is admitted, generated, published, or supported at
this commit. This document owns the milestone's identity, its gates, the
separation invariants, and the standing support/publication decision. It is a
charter, not evidence.

Audience: language, ABI, package, evidence, and promotion reviewers.

Every CI run on `main` since the recorded PG-8 commit has been cancelled
(`cancel-in-progress` plus a fast push cadence), so none of PG-5/PG-6/PG-7's
local evidence below is hosted evidence yet, and the recorded PG-8 green run
predates all of that code.

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
| PG-1 | A new versioned target-neutral type grammar: closed vocabulary, injective canonical term, byte-exact render and parse, domain-separated digest, and fail-closed rejection of every construct outside the admitted surface | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) | Hosted green |
| PG-2 | Explicit template identity and ordered argument identities: persistent template declaration identity, declared arity, positional parameter owner and index, and digests that distinguish permutation, omission, duplication, and substitution | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) | Hosted green |
| PG-3 | Semantic compatibility rules: a closed classification over two grammar surfaces with explicit reasons, no compatibility inferred from a diff classification, and no version decision inferred from a classification | [Public Generic Compatibility v1](PUBLIC-GENERIC-COMPATIBILITY-V1.md) | Hosted green |
| PG-4 | Candidate ABI-delta evidence that selects the public generic signature, retains ordered arguments and substituted fields, and survives mutation, recovery, and independent byte-exact replay | [Public Generic Candidate Delta v1](PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md) | Hosted green |
| PG-5 | Generated Rust, TypeScript/Wasm, C, and C++ consumers derived from the grammar, byte-deterministic, with no ambient authority | [Public Generic Consumers v1](PUBLIC-GENERIC-CONSUMERS-V1.md), [Public Generic Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md) | Implemented, local evidence |
| PG-6 | Hostile metadata replay: forged, stale, truncated, reordered, and mutated grammar or descriptor bytes fail closed in every consumer route and in independent replay | [Public Generic Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md), [Public Generic Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md) | Implemented, local evidence |
| PG-7 | Owned allocation and failure settlement across the boundary: bounded allocation, exact copy-out, sticky failure selection, canonical cleanup order, and equal checked behavior on interpreter, native C11, and Core Wasm | [Public Generic Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md), [Public Generic Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md); also documented in [Public Generic Consumers v1](PUBLIC-GENERIC-CONSUMERS-V1.md#cross-engine-settlement-corpus-issue-162) | Implemented, local evidence |
| PG-8 | Cross-platform hosted evidence for the complete milestone corpus on Linux, macOS, and Windows, recorded for an exact implementation commit | The `public-generic-ownership-milestone` job in [CI required checks v1](CI-REQUIRED-CHECKS-V1.md) | Hosted green |
| PG-9 | An explicit support and publication decision naming the exact version, target, and consumer scope, with its prerequisite profile decisions | This milestone | Open |

PG-1 through PG-8 are prerequisites of PG-9, not substitutes for it. A complete
set of green prerequisite gates authorizes the decision to be *made*; it does
not make it.

### Gate scope notes

A gate moves only when the whole of it is done. Where part of a gate has landed
with its own artifact and its own executable gate, it is recorded here rather
than by advancing the row.

- **PG-5 and PG-6 — grammar half hosted green, calling half now implemented
  with local evidence only.**
  [Public Generic Metadata Consumers v1](PUBLIC-GENERIC-CONSUMERS-V1.md)
  generates a Rust, TypeScript/Wasm, C, and C++ consumer of the canonical
  metadata of a candidate surface. All four are compiled warning-free and run
  for real, and all four must refuse nine hostile documents — forged term
  length, reordered records, stale surface, truncation, and the rest — with the
  same closed reason as each other and as the Rust reference reader. That part
  is hosted green (see the PG-8 note below).

  A versioned [Public Generic Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md)
  and [Logical Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md) now exist, with
  independent byte-exact replay, and all four languages generate a *calling*
  consumer (Rust #156, C11 #158, C++17 #159, TypeScript/Wasm #157) that
  invokes a real native or Wasm provider adapter and is compiled and executed
  locally (native: real `clang -O0`/`-O2` builds linked against
  `native::template::render_reference_provider`'s C output; Wasm: a real
  `WebAssembly.Instance` call). Re-run directly for this update: `cargo test
  --locked --test public_generic_native_adapter_v1` (16 passed, 3 ignored —
  the ignored cases are `#[ignore]`d sanitizer variants requiring a
  provisioned toolchain, not failures), `cargo test --locked --test
  public_generic_wasm_adapter_v1` (15 passed, 0 failed), and
  `sh tests/public_generic_native_adapter_v1/run_all_four_callers.sh` (issue
  #172's aggregate entry point; exit 0, printing one `AGGREGATE ... PASS`
  line per caller and the same honesty line reproduced above). A shared
  hostile corpus (#160) is checked
  identically across the Rust reference decoder, the native manifest, and all
  four consumers. None of this has a hosted CI run: every run on `main` since
  implementation commit `2ef043ba…` has been cancelled (`cancel-in-progress`
  plus a fast push cadence), so PG-8 has not been re-executed against any of
  it (tracked by #163). Three concrete gaps remain open and undecided:
  - every provider adapter (interpreter, native, Wasm) binds a **fixture**
    endpoint and **fixture** trusted descriptor/binding bytes, not a real
    function body generated from an admitted public-generic export — the
    codegen wiring from a verified descriptor to a callable native/Wasm
    function body does not exist yet, so no real monomorphized `.spx` public
    generic export has ever been called through this boundary end to end;
  - no compiled `.wasm` artifact implements the provider ABI (open /
    input_prepare / call / result_export / release); the TypeScript consumer
    calls a real Wasm function for the one reversal endpoint but keeps
    allocator/handle bookkeeping host-side in TypeScript, and the harness's
    own `.wasm` is a hand-assembled, committed test-only stand-in
    (`tests/public_generic_wasm_adapter_v1/reference_wasm_module.rs`), not a
    build of `src/public_generic_abi/wasm/**` (#229, human-blocked on a
    scope decision);
  - #173's remaining PG-6 scope — extra/reordered/duplicate descriptor
    fields, unknown schema/version, and stale Project/ProgramRoot/artifact
    associations exercised through all four *calling* consumers, plus
    persisted property/fuzz-minimized reproductions — is deliberately not
    covered by #160's shared corpus, which compares only the opaque
    authenticated byte-string equality check each calling consumer can
    express identically today; that structured hostility exists only at the
    Rust reference `descriptor.rs`/`carrier/frame.rs` layer.

  Both gates therefore move from `Open` to `Implemented, local evidence`, not
  `Hosted green`.
- **PG-4 — route implemented, and it now describes a genuine generic export
  when one is named explicitly (issue #139).**
  [Public Generic Candidate Delta v1](PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md)
  is candidate-bound and grammar-strict: an export is *described* only when the
  grammar spells every parameter and the result, so an entry in the delta is
  exactly a candidate public generic signature and nothing looser.

  The manifest-profile route still describes nothing generic, and that is a
  structural fact rather than a defect: every Project profile's admission
  eagerly lowers each declared export, and nested-owned-record derivation
  refuses any nominal with non-empty arguments outright, so no manifest profile
  can carry a generic `web_export` at all. The milestone's own generic fixture
  therefore still comes out all-excluded — and the gate asserts that its rendered bytes carry no template
  identity, no instance term, no record identity, and not even the grammar's
  instance sigil.

  What changed is that the classifier and descriptor producer work directly
  over the resolved program rather than over a manifest profile, so a function
  reachable from the entry closure can satisfy Boundary Profile v1 without ever
  being a `web_export`. `public_generic_delta_with_boundary_subjects` takes
  such subjects explicitly from the caller — never auto-scanned — merges them
  into the compared set, classifies each independently, and binds a real
  descriptor digest into a `facts.boundary_profile` section. The zero-subject
  call is the same route and is proven byte-identical to the original, so no
  existing report moved. The substantive record evidence still rides on a
  Project v9 record-returning export, where a real `add_record_field` change is classified
  `breaking` with the finding on the record that changed, with ordered
  arguments, substituted fields, and owned leaves retained across mutation,
  recovery, and byte-exact independent replay.
- **PG-7 — obligations specified and bound, execution now implemented with
  local evidence, cross-engine settlement partial.**
  [Public Generic Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md)
  derives, for one owned instance parameter, which owned leaves a boundary is
  accountable for, in which order, how each is discharged, and what is released
  when a transfer fails part way — and binds all of it to the compiler's own
  cleanup facts: the inventory's structural leaf order, its per-leaf liveness
  flags and drop lifecycles, and the plan's whole-parameter transfer unit. Any
  disagreement is a refusal, so a future boundary cannot quietly diverge from
  the ownership the compiler verified. [Public Generic Carrier
  v1](PUBLIC-GENERIC-CARRIER-V1.md) now adds a real boundary: a native C11
  physical adapter (#154), a Core Wasm physical adapter (#155), and a
  reference interpreter physical adapter (#162), each performing real
  allocation, transfer, copy-out, sticky failure selection, and canonical
  release against a fixture endpoint, each covered by its own local test
  suite including a full 0-13 (native) / 0-7 (Wasm) failure-injection matrix
  with zero-live-resource assertions. A shared cross-engine settlement corpus
  (`carrier::settlement_corpus`, #162) drives the interpreter and Wasm
  adapters through the identical case table and diffs semantic output,
  normalized trace, release order, and resource counters byte-for-byte — this
  is what caught two real defects this session, on both engines, where a
  physical free was performed without its matching logical trace event
  (Wasm input path, native result path; fixed and each pinned by a
  concrete-trace-contents regression test, not a bare success assertion).
  Re-run directly for this update: `cargo test --locked -p semaprax --lib
  public_generic_abi::carrier::settlement_corpus` — 9 passed, 0 failed.

  This is not yet the full PG-7 the milestone asks for:
  - **native C11 is not a party to the cross-engine corpus.** It has no
    in-process Rust adapter analogous to `InterpreterProvider`/`WasmProvider`
    — only a C-source renderer — so its real compiled `-O0`/`-O2` equivalence
    against this exact case table is not established, only asserted
    separately in its own harness (#162 tracks this as open);
  - **peak allocation/handle counters are not compared**, only final
    (post-terminal) counts, so the corpus is silent on the "peak" fields
    issue #162 itself lists as required;
  - **every adapter still binds a fixture endpoint**, not a function body
    codegenned from a real admitted public-generic export — the same gap
    named under PG-5/PG-6 above;
  - nested/multi-level owned records are not exercised in any adapter or the
    corpus; every adapter is flat-owned-`Bytes`-leaves only. Issue #119
    closed this session, but only for an unrelated internal
    owned-record-collection execution profile — wiring an admitted
    public-generic descriptor into a codegen-emitted function body, which is
    what would lift this specific limitation, remains unimplemented and is
    not what #119 covered (see the native adapter's own "Deferred scope" note
    in [Public Generic Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md)).

  The gate therefore moves from `Open` to `Implemented, local evidence`, not
  `Hosted green`: no hosted CI run exists for any of this (every run on `main`
  since `2ef043ba…` has been cancelled), and #174 ("execute settlement on all
  admitted backends") is closed only for interpreter/Wasm parity plus
  independently-compiled native, explicitly short of full cross-engine
  settlement and a hosted run.
- **PG-8 — hosted green, and what it did and did not establish.** The
  `public-generic-ownership-milestone` job runs the whole milestone corpus on
  `ubuntu-latest`, `macos-latest`, and `windows-latest`, and it is a declared
  release blocker rather than an optional lane, so it cannot be satisfied
  vacuously. It resolves the consumer toolchains each host really has and
  prints them, because a language whose toolchain is absent is skipped and a
  narrower run must be visible rather than read as a pass. All three legs are
  green for implementation commit `2ef043ba1b989f49b256e456f71fb6e89068bf33`
  in [run 34594793245](https://github.com/wavect/semaprax/actions/runs/34594793245) — jobs 103248047092 (Linux), 103248046648
  (macOS), and 103248046983 (Windows) — and each leg's log records all four
  consumer toolchains, `rust`, `typescript`, `c`, and `cxx`, as exercised
  rather than skipped. This is hosted evidence for the corpus those gates
  own. It is not evidence for PG-5's calling consumers, PG-6's descriptor
  bytes, or PG-7's boundary settlement, none of which the corpus contains.

  Getting there found three real defects that a Unix-only run could not: the
  Windows checkout failed before any gate for want of `core.longpaths`; a CRLF
  checkout of the generator's templates silently stopped every placeholder from
  substituting, so generated consumers lost their declarations and embedded
  metadata; and the Windows UCRT's deprecation of the standard `fopen` broke
  the generated C and C++ consumers' `-Werror` build. Each was fixed at its
  cause and each is now pinned by a gate.

  This hosted run predates, and the job as it stands today still does not
  run, any of PG-5/PG-6's calling-consumer work (issues #156–#160, #172,
  #173) or PG-7's cross-engine settlement corpus (issue #162): confirmed by
  reading `.github/workflows/ci.yml` directly, the job's "Four-language
  metadata consumers and hostile replay" step is unchanged and still invokes
  only `cargo test --locked -p semaprax --test projections
  public_generic_consumers`. Extending this required job to cover the new
  work is the next step this milestone still owes; it is out of this update's
  file lease (`.github/workflows/**`), so the exact delta is recorded in
  `HANDOFF.md` instead.

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

## Where the milestone stands

Five gates are hosted green on Linux, macOS, and Windows for one exact
implementation commit. Three more (PG-5, PG-6, PG-7) have real code and
passing local gates but no hosted run. One (PG-8) needs a fresh run once
those three are ready to claim. PG-9 is undecided.

| Gate | Artifact | What remains |
| --- | --- | --- |
| PG-1, PG-2 | [type grammar](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) | nothing; hosted green on three hosts |
| PG-3 | [compatibility rules](PUBLIC-GENERIC-COMPATIBILITY-V1.md) | nothing; hosted green on three hosts |
| PG-4 | [candidate delta](PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md) | nothing; hosted green on three hosts. It describes a genuine public-generic signature only when one is named explicitly via `public_generic_delta_with_boundary_subjects` (#139, #161); no manifest-profile route admits one on its own |
| PG-5, PG-6 | [consumers](PUBLIC-GENERIC-CONSUMERS-V1.md), [descriptor](PUBLIC-GENERIC-DESCRIPTOR-V1.md), [carrier](PUBLIC-GENERIC-CARRIER-V1.md) | a hosted run; codegen wiring from a verified descriptor to a real callable function body on any backend (every provider still binds a fixture endpoint); a compiled `.wasm` implementing the full provider ABI (#229); #173's remaining descriptor-level hostile cases exercised through all four calling consumers, plus MSRV and the 16 MiB bound for foreign consumers (#226) |
| PG-7 | [settlement obligations](PUBLIC-GENERIC-SETTLEMENT-V1.md), [carrier](PUBLIC-GENERIC-CARRIER-V1.md); [cross-engine corpus](PUBLIC-GENERIC-CONSUMERS-V1.md#cross-engine-settlement-corpus-issue-162) | a hosted run; native C11 joining the cross-engine settlement corpus (#162); peak allocation/handle counters; the same fixture-endpoint and nested-record limitations as PG-5/PG-6 above |
| PG-8 | the `public-generic-ownership-milestone` CI job | a fresh run at the exact commit where PG-5/PG-6/PG-7 land, once `main`'s CI stops being cancelled before completion; the recorded green run predates all of PG-5/PG-6/PG-7's code |
| PG-9 | this document | the decision itself, once the eight above are hosted green |

The shape of what is left is no longer "there is no versioned public generic
descriptor and carrier" — one now exists, with local evidence for all three
of PG-5, PG-6, and PG-7. What is left is: no code path anywhere compiles a
real function body from an admitted public-generic export (every physical
adapter still calls a fixture endpoint), no compiled Wasm artifact implements
the provider ABI, and no hosted run has ever exercised any of it.

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
