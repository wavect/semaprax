# Roadmap

The roadmap follows risk, not feature spectacle. Stable semantic editing, ownership inference, and component boundaries are more important than accumulating syntax.

## 0.1 — Executable semantic seed

Status: implemented in this repository.

- Canonical source and typed expression core.
- Stable declaration identities.
- Revisioned semantic graph and context slices.
- Effects, module permits, and contract guards.
- Machine-readable diagnostics.
- Atomic, stale-safe semantic renames.
- Checked native code generation.

## 0.2 — Useful core language

Status: in progress. Resource ownership boundaries and explicit
lifecycle/interface contracts, lexical `let`, typed `if/else`, partial-place
diagnostics, record syntax/checking, Graph v6, validated stable-ID HIR/type
facts, a mandatory replay-validated cleanup plan, versioned normalized-status
and semantic-event-dictionary types, native scalar status/out execution, a
browser-loadable scalar Wasm backend, and one narrow direct-trivial-resource
Wasm owned ABI are implemented. A private generated-callable native host now
connects the exact loader, authority, ledger, strict codecs, event dictionary,
and compiler-owned trace-path certificate. That host and the real Wasm lane
match the reference outcome, complete trace, publication, and final logical
liveness for all 14 cases at native O0/O2; the remaining public and language
gates below are not.

- Records, variants, `Option`, and `Result`.
- Exhaustive pattern matching.
- Generic functions with constraints.
- Modules, imports, and multi-file graph commits.
- First-class diagnostic repair operations.
- Property tests generated from types and contracts.
- A persistent graph daemon and JSON-RPC agent transport.
- Complete ownership/lifetime/region analysis across control flow.

Exit criterion: build a non-trivial CLI and edit it entirely through semantic transactions.

The aggregate tranche is specified in [RFC
0002](RFC-0002-ALGEBRAIC-DATA.md). RFC 0003 phases 1–2 now supply explicit
trivial/imported lifecycle syntax, declaration-only interface/import contracts,
source/HIR validation, and a target-neutral cleanup plan. Resolved functions
carry typed blocks/edges/regions/exits, guarded liveness, atomic call commits,
sticky status sources, cleanup order, and result publication; validation
independently rebuilds the plan, and Graph v6 serializes it.

Phase 3 now composes its formerly separate native evidence layers for the
private direct-trivial slice. Feature-gated compiler emission produces the
complete generated provider and descriptor v2; compile-time guards prove the C
compiler's architecture/OS/environment/object/endian profile or fail closed.
The host independently authenticates descriptor bytes, strict wire codecs,
dictionary, and the separately fingerprinted compiler trace-path trie-DFA,
then invokes the exact loader-instance callable through the same-thread
authority and atomic ownership ledger. Real generated shared libraries at O0
and O2 match the reference executor for all 14 scenarios, as does real
Node/Wasm.

This proves the narrow private semantics, not the public native boundary.
Compiler resource builds retain `SPX-B104` while physical/malformed-response
fallback cleanup and quiescence remain nongeneralized and Android/iOS device or
static-link profiles are absent. The pinned-nightly Rust-host ASan lane and the
full Linux/macOS/Windows matrix are green in [public run
31259216533](https://github.com/wavect/semaprax/actions/runs/31259216533); the
Rust-host evidence is the narrower [job
93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065).
The public build-only API/CLI now emits one selected direct-trivial callable-v2
bundle with deterministic hashed metadata, but it exposes no execution,
admission, or adoption surface.
Imported lifecycles, calls, aggregates, broader control flow,
cross-realm/worker identity, aggregate layouts, variants, matching,
concurrency, and fork recovery remain subsequent work.

Callable v3 is a separate bounded physical tranche: graph-derived providers
execute all 14 normal corpus scenarios at `-O0`/`-O2`, and one narrower joint
path connects scalar discard-two and owned identity through the private desktop
loader to the OS-seeded receipt ledger. The next gate puts the remaining
scenarios through that joint path and adds exhaustive failure/malformed/
allocation injection, publicly observed sanitizer and Windows runtime,
Android/iOS, and quiescence. None of these private steps opens public admission
or `SPX-B104`.

The model-backed, proposed [RFC 0004 native call recovery and settlement
contract](RFC-0004-NATIVE-CALL-SETTLEMENT.md) specifies the bounded linear
frame, certified checkpoint, idempotent settlement, receipt, and quiescence
model proposed for the physical-failure blocker. The hidden target-neutral model
and private compiler derivation from validated cleanup HIR now exist for the
current direct-trivial owned slice. A private `SPXNPRF1` proof envelope and
independent semantic parser bind that graph to the exact callable-v2 contract;
this grants no authority. The separate private [callable ABI
v3](NATIVE-CALLABLE-ABI-V3.md) fixes the descriptor/hash/graph/capacity and
linkage metadata plus seven complete, independently encoded/parsed runtime
roles: six provider wires and one host-only HMAC receipt. Its six-argument
execute ABI, payload-bearing frame, tags, digest DAG, and changed private known
answers are frozen. `CertifyOutcome` binds its embedded ordinal/outcome witness
to the
trace-certificate fingerprint through a nonzero host-recomputed digest and
rejects resealed mutations; this is not independent host acceptance of the
trace-path DFA certificate. The emitter is bound to its compiler build target
with no Android/iOS/Windows cross-emission or runtime evidence; future iOS
device, simulator, and macabi targets remain distinct. Private graph-derived
providers execute all 14 normal scenarios at `-O0`/`-O2`; the desktop v3 loader
binds exact descriptor bytes and all entry points to one root image; and the
host provides exact-instance receipt authority, authoritative owner
generations, atomic receipt/ledger publication, cached replay, and drop-safe
quarantine. One joint path now covers scalar discard-two and owned identity.
The remaining scenarios, static/mobile execution, and public compiler
admission stay closed with `SPX-B104`.

With the current private descriptor-v3 metadata defined, the hidden settlement
model starts at
the authenticated post-`CallCommit` boundary and makes one exact
`SettlementDecisionCommit`, provider settlement, and model `ReceiptCommitted`
eligibility executable.
Its 29 focused tests prove pre-decision unwind selects `Abort(HostUnwind)`,
post-decision unwind resumes the locked decision, every-finalizer interruption
quarantines without retry, candidate/committed replay is exact, and hostile
phase mutations preserve evidence. The private physical path proves
exact-instance reservation, allocation-free `CallCommit`, host-only
receipt authentication, one authoritative ledger publication, refreshed owned
generations, infallible pre-reserved quarantine on postcommit drop, and a
two-fixture provider/loader/host composition. Next, the other 12 normal
scenarios must cross the same boundary and all paths must receive failure,
malformed, allocation, sanitizer, Windows, mobile, and quiescence evidence.
The current host still allocates while parsing/replaying postcommit evidence;
pending/pre-execute unwind fails closed until its canonical transcript is
frozen. The pure model itself grants none of that authority and does not open
`SPX-B104`.

The dedicated Linux
[dynamic-provider sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 generated-provider cases through the host. It linked
the sanitizer runtimes without instrumenting Rust host code; unrelated
Clippy/GCC failures kept the overall workflow run red, so Windows and the full
platform matrix were unproven by that historical run. The later narrow Windows
callable/dependency-isolation run is green; broader application-platform
completion remains open.

The active phase-3 gate is [Owned resource vertical slice
v1](OWNED-RESOURCE-VERTICAL-V1.md). It requires one production-reachable,
thread-confined native host and one instance-confined Wasm host to execute the
same admitted direct-trivial-resource cleanup plan with exact reference-trace
equivalence. The private host now meets that semantic corpus requirement. The
remaining fallback cleanup/quiescence, public execution/admission, and mobile-
profile requirements keep the gate open, and every broader resource or
aggregate shape remains closed. Rust-host ASan instrumentation is green in the
public run cited above.

## 0.3 — Ownership and fast development

- Values, unique ownership, borrowed views, and regions.
- Escape analysis with actionable lifetime diagnostics.
- Explicit shared immutable reference counting.
- Restricted `unsafe` modules and review summaries.
- Cranelift JIT/AOT development backend.

Exit criterion: implement a zero-copy parser and server without a tracing GC.

## 0.4 — Components and packages

- Interface-first package format and target matrices.
- WIT import/export and WebAssembly Component output.
- Portable canonical ABI plus native fast ABI.
- Generated C headers and safe wrapper annotations.
- Capability-sandboxed reproducible package builds.
- Provenance, SBOM, license, and unsafe-code metadata.

Exit criterion: compose SEMAPRAX, Rust, and JavaScript components behind one interface contract.

## 0.5 — Concurrency and applications

- Structured tasks, cancellation, and deterministic scheduling.
- Effect handlers for deterministic tests.
- Application state and semantic UI dialects.
- DOM/CSS server rendering and hydration.
- Platform adapters beginning with web, then Apple and Android.

Exit criterion: ship one offline-first web/mobile validation application with shared logic and native escape hatches.

## 1.0 criteria

- Versioned language, graph, package, and component specifications.
- Reproducible native and component builds on supported targets.
- Stable debugger and profiler integration.
- Audited ownership and unsafe boundaries.
- Compatibility policy and migration tooling.
- At least one production validation system maintained across releases.
