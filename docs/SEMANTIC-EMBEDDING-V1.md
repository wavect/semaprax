# Semantic Embedding v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors integrating semantic search or retrieval
over SEMAPRAX programs, plus compiler contributors working on issue #203
("publish a small stable Semaprax embedding API with explicit host
capabilities") and its dependency #200.

Semantic Embedding v1 (`../src/semantic_embedding/`) is a small,
capability-gated boundary for computing a vector representation of
caller-supplied bytes: an explicit [`EmbeddingCapability`](../src/semantic_embedding/capability.rs),
an injected [`EmbeddingProvider`](../src/semantic_embedding/provider.rs)
trait a real deployment binds to an actual model transport, and a kernel
enforcement function
([`kernel::embed`](../src/semantic_embedding/kernel.rs)) that checks
cancellation and a caller-declared input-size ceiling before a provider is
ever reached, then re-validates a settled vector's length and finiteness
before trusting it.

## What this is a slice of, and what it is not

Issue #203 asks for a much larger surface: opaque compiler/session
handles, explicit Source/Project load and refresh, check/format/graph/
query/context, candidate validate/replay, deterministic interpreter
execution, version/feature negotiation, and eventually a C ABI. None of
that lives in this module or this document. This is one narrow,
honestly-scoped slice: the "explicit provider/capability injection" bullet
of #203's "In scope" list, applied specifically to computing an embedding
vector, because that is the one sub-surface of #203 this tranche's file
lease and no-new-dependency constraint could deliver with real,
demonstrable evidence rather than an unverifiable broader claim. See
"Honesty bar" below for why a narrower, honestly labelled deliverable was
chosen over a wider one that could not be backed by evidence.

## Why a provider seam, not a real model call

Two things make a live model-backed embedding call impossible to
demonstrate honestly in this environment:

1. **No model budget is provisioned.** Issue #112's `HUMAN_BLOCKED` item
   names this directly; this tranche makes no live paid model call and
   designs nothing that requires one to test.
2. **No new Cargo dependency is permitted** for this tranche, so this
   module cannot link an HTTP client, an embedding SDK, or a math/BLAS
   crate. [`solver::run`](../src/assurance_manifest/smt_discharge/solver.rs)
   sets the repository precedent for the alternative when a capability
   needs a real external process: shell out to something explicitly
   provisioned rather than link it in. A network-backed embedding
   provider would follow the same shape (an explicit endpoint/credential
   the *host* supplies, never a hardcoded location or ambient discovery),
   but implementing and testing one against a real endpoint is exactly the
   live-call spend this tranche is instructed not to make.

Given both constraints, the deliverable that can be honestly demonstrated
today is the capability-gated boundary itself, plus a deterministic
fixture provider proving the boundary's enforcement rules — never a claim
that real embeddings were computed.

## The capability

[`EmbeddingCapability`](../src/semantic_embedding/capability.rs) has no
`Default` and no constructor that does not name why the grant exists.
`kernel::embed` takes it as a required parameter, so a caller cannot reach
a provider without holding one — enforced by the function's signature, not
by a runtime check that code could route around. This mirrors
`live_invocation::model_invoke::ModelInvokeCapability`'s shape exactly (see
that module for the same pattern applied to the `model.invoke` effect).

## The request/outcome vocabulary

[`EmbeddingRequest`](../src/semantic_embedding/request.rs) carries the
exact input bytes, an explicit model/policy binding identity, the exact
expected vector length (`dimensions`), and an explicit input-size ceiling
(`max_input_bytes`). Every field is caller-supplied; nothing here is
discovered from the filesystem, environment, or network. `EmbeddingRequest
::digest` produces a stable `sha256:`-prefixed canonical digest sensitive
to every field (`request::tests::digest_changes_with_every_field_
independently` locks this in field by field).

[`EmbeddingOutcome`](../src/semantic_embedding/request.rs) is either
`Settled(Vec<f32>)` or a closed `Failed { failure: EmbeddingFailure,
attempted_bytes: usize }`. [`EmbeddingFailure`](../src/semantic_embedding/request.rs)
is a closed six-case vocabulary (`Timeout`, `Cancelled`,
`CapacityExceeded`, `ProviderError`, `MalformedResponse`, `Refused`)
matching `live_invocation::model_invoke::ModelFailure`'s shape; a real
provider is expected to normalize whatever a transport reports into
exactly one of these before returning, never leak provider-specific detail
through this boundary.

## The kernel's enforcement order

[`kernel::embed`](../src/semantic_embedding/kernel.rs) checks, in order,
entirely without invoking the provider:

1. `cancelled()` — a caller-supplied `&dyn Fn() -> bool` — returns `true`:
   `Failed { Cancelled, attempted_bytes: 0 }`.
2. `request.input.len() > request.max_input_bytes`:
   `Failed { CapacityExceeded, attempted_bytes: request.input.len() }`.

Only after both checks pass does the provider run. Its result is then
re-validated, never trusted blindly:

3. A settled vector whose length disagrees with `request.dimensions`
   becomes `Failed { MalformedResponse, .. }`.
4. A settled vector carrying any non-finite (`NaN`/infinite) component
   becomes `Failed { MalformedResponse, .. }`.

`src/semantic_embedding/tests.rs` exercises all four checks, including
proving (via `ScriptedEmbeddingProvider::must_not_be_called`, which panics
if reached) that checks 1 and 2 genuinely never call the provider.

## The fixture provider

[`FixtureEmbeddingProvider`](../src/semantic_embedding/fixture.rs) is the
only `EmbeddingProvider` this crate ships that is meant to stand in for a
real one in tests. It is **not a model** and must never be presented as
one. Each `f32` component is constructed as follows, with **zero
floating-point arithmetic instructions executed**:

1. Compute `request.digest()` (a sha256-based, domain-separated digest of
   every request field).
2. For component index `i`, compute a second domain-separated sha256
   digest of `"<request digest>:<i>"`.
3. Take the first 4 bytes of that digest as a `u32` word.
4. Build the IEEE-754 binary32 bit pattern directly:
   `sign = bit 31 of the word`, `exponent = 127` (the fixed bias,
   representing `[1.0, 2.0)`), `mantissa = the word's low 23 bits`.
5. Reinterpret those bits as an `f32` via `f32::from_bits` — a bit-pattern
   reinterpretation, not a computed value.

Every component is therefore finite and lies in
`(-2.0, -1.0] ∪ [1.0, 2.0)`: never zero, subnormal, infinite, or `NaN`.

### Determinism: what is and is not guaranteed

**Guaranteed, and tested as a known-answer regression
(`fixture::tests::fixture_component_is_a_stable_known_answer`,
`request::tests::digest_is_a_stable_known_answer_for_a_fixed_request`):**
byte-for-byte identical output for byte-identical `EmbeddingRequest`
fields, across every host, architecture, run, and rebuild this compiler
targets. This holds specifically *because* no floating-point summation,
multiplication, division, or transcendental function (`sin`, `exp`,
`log`, `powf` — the classic sources of cross-platform, cross-library-
version, and summation-order float nondeterminism) ever executes: the only
computation is sha256 (pure integer/bitwise) followed by a bit-pattern
reinterpretation. `f32::from_bits` performs no rounding and has no
implementation-defined behavior. The one assumption this relies on —
that the target's `f32` is IEEE-754 binary32 — holds for every host
SEMAPRAX currently targets.

**Not guaranteed, and never claimed:** anything about what a *real*
trained embedding model would return. A real provider's output is
ordinarily **not** guaranteed byte-for-byte reproducible across host,
library version, or even repeated calls with identical inputs, because
real embedding computation typically involves floating-point summation
over a non-fixed reduction order (batched matrix multiplication,
SIMD/GPU-parallel accumulation), and frequently a hardware-fused
multiply-add path or a vendor math library whose rounding can differ from
a reference implementation bit-for-bit even when every input and every
declared weight is identical. A caller integrating a real provider behind
[`EmbeddingProvider`] must not assume the resulting vectors are
byte-reproducible across provider/library/hardware upgrades — only that,
for that provider's own declared reproducibility guarantee (if any), the
kernel's shape/finiteness validation and the capability gate still apply
unchanged.

[`ScriptedEmbeddingProvider`](../src/semantic_embedding/fixture.rs) is a
separate, non-model fixture: a scripted queue of `EmbeddingOutcome`
values a test hands it ahead of time, used only to exercise the kernel's
malformed-response and pass-through-failure paths, which the pure fixture
above (which always settles) cannot reach.

## The no-cast / no-`f64`-at-the-boundary limit, and what it costs

Two real language limits, confirmed this session while auditing the JSON
family (#63), constrain what an embedding API can expose to *checked*
SEMAPRAX source (as opposed to this Rust host API):

- **There is no numeric cast in SEMAPRAX.** No `as`-style conversion
  operator exists between numeric types; none of `docs/AGENT-QUICK-
  REFERENCE.md`, the language tour, or RFC 0001 name one, and there is no
  syntax for it in the grammar.
- **No workspace Project profile admits `f64` as a parameter or a return
  type.** `docs/STANDARD-LIBRARY-V1.md`'s `std.data.json.token` row states
  this precisely: `f64` literals, arithmetic, and comparison are admitted
  inside a function body and implemented on all three backends, but
  `useful_data_workspace_parameter_admitted` and
  `useful_data_workspace_return_admitted` in `src/hir/workspace_link.rs`
  reject any Project-exported function carrying one as a parameter or
  result, with `SPX-G174`.

**What this costs a checked-source-facing embedding effect (not built by
this tranche, but the real constraint any future one must design around):**
an embedding vector could not cross a Project boundary as `Vec<f64>` or
`Vec<f32>` return values, and — because there is no cast — a SEMAPRAX
program could not itself convert a returned encoded value into a float
either. A future effect boundary exposing this capability to checked
source would have to encode each component as something Project *does*
admit today (`i64`/`i32`/`u8`/`bool`/`char`, or `Bytes`/`String`): for
example, each `f32` component's raw 4-byte IEEE-754 bit pattern carried as
`u8`/`Bytes`, or a fixed-point integer scaling scheme the effect's own
contract defines and documents (not a generic float encoding, since
generic float-to-fixed-point conversion is itself the missing numeric
cast). This module deliberately does not attempt that encoding: it is a
Rust-host-only API today, not wired into any checked-source effect
boundary, and designing that wire format is future work for whichever
tranche does that wiring, informed by this exact constraint.

## No ambient authority

Nothing in `src/semantic_embedding/` opens a file, spawns a process, reads
an environment variable, or contacts a network.
[`EmbeddingCapability::grant`](../src/semantic_embedding/capability.rs)
must be called explicitly before `kernel::embed` can run at all, and
[`FixtureEmbeddingProvider`](../src/semantic_embedding/fixture.rs) — the
only provider this crate ships — touches nothing outside the request
bytes it is handed.

## Evidence

Local, offline unit and kernel-level tests (`cargo test --locked -p
semaprax --lib semantic_embedding`) cover: the capability carrying exactly
the reason it was granted with; the request digest's stability, per-field
sensitivity, and byte-identity for byte-identical requests; the fixture's
bit-construction known-answer values; every component's finiteness and
documented range; cross-run and cross-instance bit-identical determinism
for the same request; distinct vectors for distinct input bytes; the
kernel refusing before dispatch on cancellation and on an oversized input
(proved via a provider that panics if reached); rejecting a
wrong-length or non-finite settled vector as malformed rather than passing
it through; and an unrelated provider failure passing through unchanged.
No test in this module spawns a process, opens a network socket, or makes
a paid model call. See the top-level report for this tranche's exact
command and count.

## Honesty bar

A narrow capability-gated boundary with a deterministic, clearly labelled
fixture is what this tranche delivers, in preference to a broader API
whose "real embedding" behavior could not be demonstrated without a
provisioned model budget this tranche does not have. Nothing in this
module, its tests, or this document claims real semantic-similarity
behavior, cross-provider compatibility, or any guarantee about a live
model's output.
