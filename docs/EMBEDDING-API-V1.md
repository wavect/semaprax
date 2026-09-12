# Embedding API v1

Status: versioned bounded reference; the completion matrix owns product
status. First real slice of issue #203's compiler-embedding surface.

Audience: host authors (editors, build systems, services, applications)
embedding SEMAPRAX analysis in-process, and compiler contributors extending
`src/embedding_api.rs`.

## Read this first: two modules both cite issue #203, for different reasons

`src/semantic_embedding/` (documented in
[Semantic Embedding v1](SEMANTIC-EMBEDDING-V1.md)) and `src/embedding_api.rs`
(this document) both carry `#203` in their history, and that is a genuine
naming collision worth stating plainly rather than leaving implicit:

- **`semantic_embedding`** computes a **vector embedding** (bytes → `Vec<f32>`
  via an injected model provider) — the machine-learning sense of the word.
  Its own document says outright: "Issue #203 asks for a much larger
  surface... None of that lives in this module." It is a real, tested,
  capability-gated boundary, but it answers only issue #203's "explicit
  provider/capability injection" bullet, applied to one narrow sub-case the
  issue never actually names (a vector-embedding effect). It does not touch
  compiler/session creation, source/Project load, check, or any of issue
  #203's other seven "in scope" bullets.
- **`embedding_api`** (this module) is the **compiler-embedding** sense of
  the word issue #203's title and body actually describe: "editors, build
  systems, services, and applications embed parsing, checking,
  interpretation, semantic query, candidate validation, and selected
  execution without spawning the CLI or receiving ambient host authority."

Concretely: a host wanting the vector-embedding boundary imports
`semaprax::semantic_embedding`; a host wanting to check SEMAPRAX source
in-process imports `semaprax::embedding_api`. Neither module re-exports or
depends on the other.

## What issue #203 asks for, and where each part actually stands on `main`

Issue #203's "In scope" list, mapped to real code, as of this tranche:

| In-scope bullet | Status before this tranche | Status after this tranche |
| --- | --- | --- |
| Compiler/session creation | Not exposed as a small stable API. `src/project/semantic_service.rs`'s `SemanticWorkspaceService::open` exists but requires an already-built `Arc<ProjectRevision>` — a Project-level session, not a single-unit embedding entry point, and not documented as issue #203's answer. | Still not attempted here. [`embedding_api::check_source`](../src/embedding_api.rs) is deliberately stateless (no handle, no `open`/`close`) — see "What this tranche does not do" below. |
| Source/Project load and authenticated refresh | `SemanticWorkspaceService`/`ProjectSnapshot` do this for a full Project (multi-file, manifest-driven). No single-compilation-unit, dependency-free load entry point existed as a named public embedding surface. | [`check_source(unit_name, source)`](../src/embedding_api.rs) loads exactly one caller-supplied unit from explicit bytes; no manifest, no multi-file Project. |
| Check | `crate::check` (crate root) and the CLI `check` command both exist, but neither is documented as a stable embedding surface, and `crate::check` silently discards non-error diagnostics on a successful check. | `check_source` is that documented surface for one unit, and deliberately keeps every diagnostic (warnings included) on success — see "Diagnostics are never discarded on success" below. |
| Format, graph/query/context | `format::canonical`, `graph::to_json`, `graph::context_json` already exist as public functions, unchanged by this tranche. | Not extended here; a real gap remains: none of these are re-exposed through a versioned embedding facade with the same panic-normalization and revision-hash guarantees `check_source` now has. Named under "What remains," not claimed done. |
| Candidate validate/replay | `src/project/candidate/**` implements this for the Project workspace transaction path (out of this tranche's lease: `src/live_invocation/**`, `src/agent_runtime_v2/**` are explicitly off-limits). | Not attempted; out of lease. |
| Deterministic interpreter execution for admitted profiles | `interpreter`/`hosted_interpreter` exist; no capability-gated embedding entry point wraps them. | Not attempted here. A future execution slice needs its own explicit capability type (see "What this tranche does not do"). |
| Explicit provider/capability injection | Done, for the vector-embedding effect only, by `semantic_embedding::EmbeddingCapability`/`EmbeddingProvider` (see above). | `check_source` needs no capability because checking is pure and effect-free; this bullet is satisfied for *this* operation by construction (nothing to inject authority into), not by adding an unnecessary capability type. |
| Memory/resource ownership and cancellation | Not documented for any embedding surface. | `check_source` holds no resource across calls (stateless, no handle to leak or double-free) and completes in one bounded call, so there is nothing to cancel; this is a property of the operation's shape, not a cancellation mechanism, and is stated as a nonclaim, not a delivered mechanism. |
| Version/feature negotiation | Not present for any embedding surface. | [`EMBEDDING_API_VERSION`](../src/embedding_api.rs) and `EmbeddingApiVersion::is_compatible_with` exist and are tested against both a matching and two non-matching major versions. |
| Thread-safety and reentrancy contract | Not documented. | `check_source` takes no shared or mutable state; every call is independent and safe to run from any number of threads concurrently (ordinary Rust `&str`-in, owned-value-out; no interior mutability, no global, no lock). |

## The `CheckOutcome` contract

[`CheckOutcome`](../src/embedding_api.rs) is the only value `check_source`
returns. It carries `unit_name` (echoed back, never read from disk),
`ok: bool`, `diagnostics: Vec<Diagnostic>`, and `revision: Option<String>`
(present exactly when `ok` is `true`). It deliberately never carries
[`crate::ast::Program`], [`crate::hir::Analysis`], or
[`crate::hir::ResolvedProgram`] — issue #203's "Validated internals remain
compiler-owned" acceptance criterion applied literally: an embedder gets a
report, never a value whose internal shape this crate is free to change
release to release.

### Diagnostics are never discarded on success

The crate-root [`crate::check`] helper (used throughout this repository's own
tests) returns `Ok(Program)` on success and, in doing so, throws away every
non-error diagnostic — a real embedder using that helper directly would
never see a warning on a program that still checks. `check_source` calls
[`crate::hir::analyze`] directly instead (the same function the CLI `check`
command uses) and keeps `diagnostics` in full regardless of `ok`.
`a_declaration_missing_id_still_checks_ok_but_keeps_its_warning` in
`src/embedding_api.rs`'s test module locks this in against the missing-`@id`
`SPX-S103` warning specifically.

### No ambient authority

`check_source` opens no file, spawns no process, and reaches no network.
`unit_name` labels diagnostics only; it is never opened as a path.
`unit_name_is_never_read_from_disk` proves this concretely: checking valid
source under a `unit_name` that names a path guaranteed not to exist on the
running machine still succeeds, because the function's only real input is
`source`.

### Panic normalization

A parser or analyzer defect must not unwind across this boundary into a
host's own stack. `check_source` runs the actual analysis inside
`std::panic::catch_unwind` and converts a caught panic into a
[`Diagnostic`] carrying [`PANIC_NORMALIZED_DIAGNOSTIC_CODE`]
(`"SPX-EMB001"`) — a code reserved for exactly this case and never produced
by parsing or analyzing real source. Because manufacturing an actual
parser/analyzer panic on demand would not be an honest regression fixture,
the panic path is proven with a private test-only `SourceChecker`
implementation that panics on purpose
(`embedding_boundary_normalizes_a_panic_into_a_diagnostic_never_propagating_
the_unwind`), the same seam-substitution pattern
`src/semantic_embedding/fixture.rs`'s `ScriptedEmbeddingProvider` already
uses in this repository to prove a path a pure, always-succeeding fixture
cannot reach on its own.

### Distinguishing a genuine refusal from an internal defect

`malformed_source_fails_with_the_specific_parser_diagnostic` checks a module
with no function body (`"module app.empty;\n"`), which the parser refuses
with `SPX-P101` ("a module must declare at least one function") before any
HIR analysis runs, and asserts that diagnostic's code and message never
mention `PANIC_NORMALIZED_DIAGNOSTIC_CODE`. The panic-normalization test
asserts the reverse: its diagnostic never mentions `SPX-P101`. A caller can
therefore always tell "your source is invalid" (`SPX-P101` or any other real
diagnostic code) apart from "the compiler itself broke while checking your
source" (`SPX-EMB001`) — two refusal paths that a less careful test could
conflate.

## Compatibility policy

`EMBEDDING_API_VERSION` (currently `1.0.0`) names this Rust surface's own
version, independent of any checked SEMAPRAX program's semantics.
`EmbeddingApiVersion::is_compatible_with(requested_major)` returns `true`
only when `requested_major` equals this build's `major`; a differing major
version is refused rather than silently assumed compatible.
`version_negotiation_accepts_matching_major_and_refuses_a_different_one`
tests both a match (`1`) and two refusals (`0` and `2`). Within one major
version, `check_source`'s accepted inputs (`unit_name: &str`, `source: &str`)
and `CheckOutcome`'s fields are additive-only: a future `1.x` may add a field
to `CheckOutcome` but will not remove or repurpose `unit_name`, `ok`,
`diagnostics`, or `revision`, and will not change `check_source`'s signature.
A breaking change to any of those requires bumping `major` and updating
`EMBEDDING_API_VERSION` in the same change.

## What this tranche deliberately does not do

Naming every nonclaim explicitly, per this repository's honesty-bar
convention:

- **No opaque session/compiler handle.** `check_source` is a stateless
  function; there is no `open`, `close`, handle type, or lifecycle to get
  wrong. A real multi-call embedding session (reusing parsed state across
  checks, incremental reparse) is future work and would need its own handle
  type, `Send`/`Sync` contract, and destruction rule — none of which are
  invented here ahead of a real use case that needs them.
- **No multi-file Project load.** One caller-supplied unit only. Building a
  Project (manifest, multiple files, dependency resolution) is
  `src/project/**`'s job, already versioned and documented separately
  (`docs/PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md`); this module does
  not wrap or re-expose it.
- **No execution capability.** `check_source` never runs a SEMAPRAX
  program. A future execution slice needs an explicit, non-`Default`
  capability type gating it — mirroring
  `live_invocation::model_invoke::ModelInvokeCapability` and
  `semantic_embedding::capability::EmbeddingCapability`'s shape exactly —
  and is not added here because nothing in this tranche exercises it; adding
  an unused capability type would be exactly the kind of speculative surface
  issue #203 warns against ("Do not create a parallel source of truth").
- **No cancellation token.** Checking one unit is a single bounded call with
  no long-running or externally-triggered work inside it, so there is
  nothing to cancel. This is stated as a property of the operation, not
  claimed as a delivered cancellation mechanism.
- **No C ABI.** Issue #203 explicitly sequences a C ABI after "stable owned
  string/record/result conventions are selected" for the Rust surface. This
  tranche is that Rust surface's first slice, not the ABI.
- **No candidate validate/replay.** Out of this tranche's file lease
  (`src/live_invocation/**`, `src/agent_runtime_v2/**` are explicitly
  off-limits) and out of scope for a check-only slice.
- **No re-exposure of `format`/`graph::to_json`/`graph::context_json`
  through this facade.** They remain available at their existing paths,
  unchanged; wrapping them with the same panic-normalization and version
  contract `check_source` has is real remaining work, not claimed here.

## Evidence

Local, offline unit tests only (`cargo test --locked -p semaprax --lib
embedding_api::tests`): a valid program checks with no diagnostics and a
revision; a program missing `@id` still checks `ok` while keeping its
`SPX-S103` warning; a module with no function fails specifically with
`SPX-P101`; a `unit_name` naming a nonexistent path still succeeds because
only `source` is read; a deliberately panicking test double is normalized to
`SPX-EMB001` rather than unwinding; and version negotiation accepts a
matching major version while refusing two different ones. No test in this
module spawns a process, opens a network socket, or reads a real file from
disk. See the top-level report for this tranche's exact command and count.
