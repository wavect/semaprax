# Changelog

- Added the parallel batch-1/batch-2 language and projection tranches
  (isolated worktrees under `.agent-worktrees/`, merged into `main`):
  - String operations v1 — reserved `core.string.*` intrinsic calls
    (`string_len`, `string_concat`, `string_is_empty`) admitted through the
    ordinary monomorphic call path with borrow/move ownership, native C11
    O0/O2 + Node/Wasm + interpreter equivalence, gated C helpers/host imports,
    and pinned evidence in `tests/string_ops_v1.rs`
    (`examples/string_ops.spx`, `docs/STRING-OPS-V1.md`).
  - Field mutation v1 — statement-only direct scalar Copy-field assignment
    `<binding>.<field> = <expr>;` on `let mut` record/class locals with
    diagnostics `SPX-U107`-`SPX-U112`, additive graph `"field"` attribute,
    unchanged CleanupPlan shapes, and native+Wasm equivalence in
    `tests/field_mutation_v1.rs` (`examples/field_mutation.spx`,
    `docs/FIELD-MUTATION-V1.md`).
  - Bounded while-loop v1 — `while <bool> { body }` under a Copy-scalar
    admission profile with diagnostics `SPX-T251`-`SPX-T253`/`SPX-G410`,
    program-level Graph v15 selected above v14, linearized cleanup-plan
    iteration with fail-closed liveness guards, native/Wasm loop lowering,
    fuel-accounted interpretation, and `tests/while_loops_v1.rs`
    (`examples/while_loops.spx`, `docs/WHILE-LOOPS-V1.md`).
  - Reference Interpreter scalar widening — admission widened to the full
    Copy-scalar surface (`i64`/`i32`/`u8`/`bool`/`f32`/`f64`/char) including
    suffixed/float/char argument parsing, with a 24-row cross-backend parity
    corpus in `tests/interpreter_scalar_widen_v1.rs`.
  - Interop scalar widening — `semaprax abi-report`, `semaprax c-header`, and
    `semaprax cxx-shim` admit the full Copy-scalar surface with verbatim
    native-line digests and byte-level Wasm cross-consistency in
    `tests/interop_scalar_widen_v1.rs`.
  - Schema scalar widening — `semaprax openapi`, `semaprax package-report`,
    and `semaprax ui-schema` admit the full Copy-scalar surface with
    bound-aware compat classification, layout cross-consistency, and pinned
    widened KATs in `tests/schema_scalar_widen_v1.rs`.
  - Class single inheritance v1 — `class Child : Parent { ... }` with
    ancestor-prefix layouts, static nearest-first method resolution, exact-
    signature overrides, `super.m(...)` calls, typed-let upcasts
    (`ResolvedExprKind::Upcast`), diagnostics `SPX-T227`-`SPX-T234`, and a
    cleanup-inert suffix rule keeping plan schemas unchanged; evidence in
    `tests/class_inheritance_v1.rs` (`examples/inheritance.spx`,
    `docs/CLASS-INHERITANCE-V1.md`).
  - Frozen KAT digests and HIR capacity pins are re-pinned for honest
    AST/HIR builder-bytes growth from the new syntax; programs without the
    new constructs keep byte-identical projections.
- Added the String + Object-Oriented Types large-implementation tranche
  (branch `feat/string-oo-types`, worktree `.agent-worktrees/string-oo`):
  - Owned heap `string` value type end-to-end — `"…"` literals with canonical
    escaping, `Type::String`/`ResolvedType::String` (copy=false,
    needs_drop=true, `owned:string` facts), move/borrow ownership, cleanup
    plans, graph nodes, deterministic canonical round-trip, native C11 O0/O2,
    browser/Wasm, and interpreter equivalence with pinned KATs in
    `tests/string_scalars.rs` (`examples/strings.spx`). String arithmetic and
    concatenation are rejected fail-closed until the allocation tranche.
  - `class` declarations with fields, methods, construction, and static
    method dispatch end-to-end — canonical projection, additive graph nodes,
    Native64/Wasm32 sequential layouts, native+Wasm lowering, and
    `tests/class_declarations_v1.rs` (`examples/classes.spx`). Inheritance
    syntax is rejected fail-closed with a stable diagnostic until Badge 4.
  - `protocol` declarations + read-only Protocol Projection v1 —
    `semaprax protocol-check <file>` emits one digest-authenticated canonical
    envelope; conformance stays explicitly empty; program-graph schema stays
    v10–v14 with protocol nodes explicitly deferred;
    `tests/protocol_projection_v1.rs`.
  - Frozen workspace/workspace-graph KAT digests are re-pinned for the honest
    builder-bytes growth caused by the new AST/HIR variants; all other bytes
    stay byte-identical for programs without the new syntax.
- Added the locally evidenced Region Structure Report v1 tranche, the first
  executable slice of the completion-matrix row "Regions/arenas", moving that
  row from Missing to Partial. The new read-only
  `semaprax region-report <file> [--max-bytes N]` command and
  `region_report` library API emit one deterministic digest-authenticated
  canonical `semaprax.region-report.v1` JSON envelope per verified module: a
  lifetime-structure report for every admitted explicit-ID monomorphic
  effect-free scalar function, derived entirely from facts the existing
  borrow/move checking already proves. Per function the report carries the
  binding lifetime partition (parameters, `let`/`let mut` locals, and match
  pattern bindings with real resolved-HIR value ids, ownership modes,
  canonical type keys, definition offsets, effective live-range ends measured
  at innermost statement/tail granularity, and use counts), canonical region
  clusters under the rule that overlapping live ranges can never share one
  region (greedily clustered in canonical binding-id order), explicit escape
  facts naming ownership check `SPX-O104` as the reason every borrow today is
  provably non-escaping, move facts recomputed from the resolved call graph
  (own-consumption sites plus derived moved bindings), and maximal
  bulk-release grouping candidates (sets of at least two bindings whose
  effective ends coincide); every non-admitted function is recorded with one
  of six closed exclusion reasons mirroring the shared scalar profile.
  `verify_envelope` independently replays bytes, counts, vocabularies,
  orderings, the full greedy clustering re-derivation from reported live
  ranges, escape/move derivations, and exact bulk-release groupings, so
  forged-but-re-signed mutations still fail closed;
  `verify_envelope_against_source` fails closed on drift. Diagnostics use the
  previously unused `SPX-L1xx` family (`SPX-L101` options, `SPX-L102`
  fail-closed budget exhaustion without truncation, `SPX-L103` consistency).
  `tests/region_report_v1.rs` pins golden KAT digests over
  `examples/calculator.spx` and `examples/meaning.spx`, proves determinism,
  exercises every exclusion reason, cross-checks every reported binding id
  against the real resolved-HIR inventory across four examples plus a
  match-pattern fixture, and covers per-field tamper rejection, budget
  exhaustion, drift binding, and CLI exit codes. This tranche implements no
  region inference, adds no region annotation syntax, introduces no arena
  runtime behavior, changes no destructor behavior, executes nothing, and
  changes no source.
- Added the locally evidenced Deterministic Scoped Task Model v1, a hidden
  target-neutral proof-data tranche (`src/scoped_tasks.rs`) in the exact
  `native_settlement` style that moves the completion-matrix row "Structured
  concurrency" from Missing to Partial. The model fixes, as executable
  evidence only: a bounded task DAG (4,096 tasks / 4,096 scopes / 65,536 edges
  / 1,000,000 work units) inside one strict scope tree where every non-root
  scope requires exactly one parent join and cross-branch dependencies are
  rejected at construction as escaping references; deterministic sequential
  scheduling in canonical stable-id order; sticky cancellation propagation in
  which cancelling a scope marks all descendants and materializes their
  cancellation before any sibling starts new work while already-started work
  drains to its scripted outcome; children-finalize-before-parents scope exit
  in exact reverse completion order; sticky first-failure selection with
  independent siblings draining and dependents of failed prerequisites
  abandoned; closed per-task `Sendable`/`Shareable` annotations recorded as
  declared intent only; and canonical JSON model/trace projections under two
  separately domain-separated SHA-256 fingerprints.
  `tests/scoped_tasks_model_v1.rs` plus seven focused module units pin four
  known-answer trace digests for join-all, mid-scope cancellation, failure
  drain with an abandoned dependent, and nested scopes; prove exact event
  sequences, cancel-during-drain, failure-beats-cancellation stickiness,
  hostile construction rejections (escape/double/orphan joins, cycles,
  duplicates, zero physical codes, bounds/work budget), hostile run operations,
  full input-permutation determinism, a byte-pinned trace projection, and JSON
  validity. It adds no language syntax, no parser/HIR/Graph/verifier/backend
  or CLI change, no runtime threads or scheduler integration, no real
  concurrent execution (task bodies are closed scripted outcomes), no
  `Sendable` checking of real programs, and no actors/reducers/synchronization
  claim.
- Added the locally evidenced Deterministic ARC Zone Model v1, a hidden
  target-neutral proof model that moves the completion-matrix row "Shared
  immutable ARC and opt-in managed zones" from Missing to Partial by fixing
  bounded proof data only — no runtime RC integration, no language syntax, no
  compiler or backend change, and no real allocation behavior. The new
  `arc_zones` library module models shared-immutable reference counting inside
  explicit opt-in managed zones: a strict zone containment tree with balanced,
  parent-first enter/exit; accounted retain/release over base-reference,
  explicit-handle, and live-payload-link strong counts with fail-closed double
  releases that keep link-anchored objects alive; exact deterministic
  finalization order — reverse construction at zone exit, cascading through
  outgoing payload links in canonical target order depth-first; closed
  cycle-participation deferral under which retained cycles (strongly connected
  components plus self-loops) reject the zone exit with one canonical
  smallest-member witness diagnostic instead of leaking silently; escape
  demotion as a deterministic rewrite rule turning a proven zone-local shared
  handle (sole unreleased base reference, zero incoming links) into unique
  ownership whose later shared use fails closed; and closed concurrency
  annotations under which every zone declares its single executing thread and
  cross-zone or cross-thread sharing requires an explicit `Shareable` mark on
  the shared object. Zones, objects, scripts, events, and traces are bounded
  (`semaprax.arc-zones-model.v1`, `semaprax.arc-zones-trace.v1`) with
  canonically ordered inventories, domain-separated SHA-256 model fingerprints
  and trace digests, byte-pinned canonical JSON, and sticky run rejection.
  `tests/arc_zones_model_v1.rs` pins four canonical known-answer trace digests
  (shared fan-out release with canonical cascade order, cycle rejection with
  smallest-member witness, escape demotion, nested-zone children-before-parents
  drains), hostile rejections (foreign-zone handle release, unbalanced zone
  exit, double release beyond outstanding references, sharing without
  `Shareable`, demoted-object shared reuse), structural construction hostility,
  determinism under inventory permutation and repeated execution, and
  JSON-validity plus domain-separation projections; seven focused module units
  cover the remaining state machine and error surface.

>>>>>>> feat/arc-zones-v1
- Added the locally evidenced Portable SIMD Eligibility Report v1 tranche,
  the first executable slice of the completion-matrix row "SIMD and GPU".
  The new read-only `semaprax simd-report <file.spx> [--max-bytes N]` command
  and `simd_report` library API emit one deterministic canonical
  digest-authenticated JSON envelope (`semaprax.simd-report.v1`) per verified
  module: a static vectorization-eligibility analysis per admitted
  explicit-ID monomorphic effect-free scalar function, derived exclusively
  from the real resolved HIR nodes. Per function the report lists every
  maximal pure straight-line arithmetic sub-expression over
  `i64`/`i32`/`u8`/`f32`/`f64` (subtrees of `+`/`-`/`*`/unary `-` whose
  leaves are plain numeric literals or projection-free numeric places) with
  element type, operator and leaf counts, the closed portable lane-operation
  sequence in post-order evaluation order, a domain-separated per-region
  SHA-256 over the exact rendered root text, and the proposed portable lane
  width (2/4/8) selected by a documented deterministic largest-feasible-first
  rule under the fixed 128-bit lane model ceilings `i64`/`f64`→2,
  `i32`/`f32`→4, `u8`→8; plus effect-freedom justification facts with exact
  call/assignment counts; plus an explicit closed ineligibility reason for
  EVERY non-covered expression — `call`, `contract`,
  `division_remainder`, `bool_mixing`, `char_operation`, `mutation_target`
  (assignment stores are recorded once and never descended),
  `computed_operand`, `control_flow`, `aggregate_operation`, `scalar_leaf` —
  and five closed function-admission exclusion reasons
  (`automatic_identity`, `generic_function`, `declared_effects`,
  `unsupported_parameter_mode`, `non_scalar_signature`; scalar signatures
  deliberately admit `i32`/`u8`/`f32`/`f64`/`bool`/`char` so their bodies are
  analyzed honestly). `verify_envelope` independently replays bytes, shapes,
  module counts, both closed vocabularies, the fixed lane model and portable
  operation table, strict stable-ID ordering, index continuity, per-region
  digests, lane-width feasibility, operator-count agreement, effect-freedom
  consistency, and fixed nonclaims. Diagnostics use the previously unused
  `SPX-V1xx` family (options `SPX-V101`, fail-closed budget exhaustion
  `SPX-V102`, envelope/HIR consistency `SPX-V103`). `tests/simd_report_v1.rs`
  proves pinned golden KATs over `examples/calculator.spx` and
  `examples/meaning.spx`, byte-identical determinism, every function and
  expression reason exercised, lane-ceiling/tie-break coverage, tamper
  rejection per digest field including forged-but-re-signed envelopes caught
  by replay, source-drift binding, budget exhaustion, CLI exit codes, and
  cross-consistency proving reported region operators equal the real
  Add/Sub/Mul/Neg HIR nodes of the same program while division entries equal
  its real Div/Rem nodes. No SIMD codegen or intrinsics are emitted, no
  SPIR-V/WebGPU/GPU kernels exist, no autovectorization is claimed about any
  backend, no target is executed, and no source is changed.
- Added the locally evidenced Reference Interpreter v1 tranche, the first
  executable slice of the completion-matrix row "Fast development lane". The
  new `semaprax interpret <file> --function <name|stable-id> [--arg
  <i64|bool literal>]... [--max-bytes N]` command and `interpreter` library
  API evaluate ONE explicitly selected explicit-ID monomorphic effect-free
  scalar function directly from verified HIR of one verified module — no
  backend toolchain, no code generation, no target execution — under a
  closed admission profile (explicit identity, monomorphic, effect-free,
  by-value direct `i64`/`bool` boundary signature for the selected function
  and every reachable callee) covering the admitted scalar surface:
  Explicit Mutation v1 `let mut`/assignment, blocks, `if`, lazy `&&`/`||`,
  strict left-to-right evaluation with sticky first-failure selection,
  checked `i64`/`i32`/`u8` arithmetic reusing the compiler's exact
  `runtime_status` table, total IEEE-754 `f32`/`f64`, char/u8/i32 locals,
  requires/ensures contracts with `result` binding, and admitted calls
  including bounded recursion. The deterministic canonical
  `semaprax.interpret.v1` envelope reports the returned value or the exact
  normalized failure status plus fuel accounting (steps used versus budget;
  exhaustion is a fail-closed capacity outcome, never a language status),
  the fixed call-depth ceiling, argument echo, source digest, and fixed
  nonclaims; `verify_envelope` independently replays bytes, shapes,
  grammars, fuel invariants, closed vocabularies, and exact compiler-owned
  status reconstruction, and `verify_envelope_against_source` fails closed
  on drift. Diagnostics use the previously unused `SPX-F1xx` family.
  `tests/interpreter_v1.rs` proves a 28-row corpus produces byte-identical
  result/status transcripts across the interpreter, native C11 O0/O2, and
  Node/Wasm (full scalar surface versus native; the whole-program
  web-profile subset versus all three), plus pinned golden/fuel KAT digests,
  determinism, every admission reason, argument diagnostics, per-field
  tamper rejection including re-signed forgeries, drift binding, and CLI
  exit codes. No JIT/AOT/Cranelift, incremental persistence, hot reload,
  debugger mapping, or target execution is claimed.
- Added Unsafe Boundary Mechanics v1, a bounded end-to-end language slice
  that moves the completion-matrix row "Restricted `unsafe` and raw memory"
  from Missing to Partial by proving boundary mechanics ONLY: a new statement
  form `@audit("<summary>") unsafe { .. }` wraps ordinary safe checked
  statements plus one required final value expression - no raw pointers or
  memory operations exist in the language and none are added, and block
  contents are verified by exactly the same rules as safe code. Each unsafe
  block requires an audit summary attribute following the existing attribute
  syntax pattern, recorded verbatim, and the enclosing module must declare
  the capability through the unchanged module permit mechanism with the
  reserved name `unsafe` (`permit { unsafe }`); without it compilation fails
  through the previously unused `SPX-N1xx` diagnostic family: missing
  capability declaration (`SPX-N101`), missing audit annotation (`SPX-N102`),
  empty or non-string audit summary (`SPX-N103`), non-scalar/non-Copy body
  results (`SPX-N104`, so discarding introduces no ownership or cleanup
  semantics), and boundaries inside contract expressions (`SPX-N105`). The
  canonical formatter renders multi-line and inline forms byte-stably;
  HIR carries `ResolvedStatement::Unsafe` through both iterative and
  recursive resolution/validation paths without changing frame budgets;
  Graph JSON adds one explicit `"kind":"unsafe"` node per boundary carrying
  the verbatim audit string in full and compact serializers without touching
  schema selection (boundary-only programs stay at v10) or any pinned
  non-boundary bytes (the Explicit Mutation v1 digest still holds);
  CleanupPlan v2 output is structurally identical to the plain
  block-equivalent form; native C11 lowers the body transparently (scalar
  result discarded) verified at O0/O2 including checked-overflow failure
  statuses inside boundaries, and the Wasm core lane emits the body's
  instructions plus `drop` with identical Node-executed results and overflow
  trapping. Evidence lives in `tests/unsafe_boundaries_v1.rs` (13 tests).
  Non-claims: no raw pointers or memory operations, no lint/platform
  conformance coverage, no safety claims about block contents, and no
  capability machinery beyond the single compile-time gate.
- Added the locally evidenced Plugin Manifest Projection v1 tranche, the
  first executable slice of the completion-matrix row "Plugins". The new
  read-only `semaprax plugin-manifest <file> [--max-bytes N]` command and
  `plugin_manifest` library API project one verified module into one
  deterministic canonical `semaprax.plugin-manifest.v1` envelope describing
  a capability-limited plugin descriptor: a sorted provided-export inventory
  of explicit-ID monomorphic effect-free by-value `i64`/`bool` functions,
  each with its persistent stable ID, interface parameter/result types,
  canonically rendered requires/ensures clauses, and the exact Native64
  prototype line extracted verbatim from the production native C11
  projection under a per-export domain-separated digest; every other
  function is recorded under one of six closed exclusion reasons mirroring
  Canonical ABI Report v1. Plugin identity fields follow the existing module
  metadata conventions — the module declaration name plus a build-hash-style
  version derived from the domain-separated stable source digest, because
  the language has no version metadata today. The required host capabilities
  section reuses the Build Capability Manifest v1 derivation and helpers
  over the same closed five-domain vocabulary (module permits plus declared
  function and interface-import effects; unconsumed interface permits stay
  checked-but-not-declared), failing closed with `SPX-Q102` on any
  out-of-vocabulary token. An explicit empty-by-default canonical
  resource-limits section, a closed five-entry unavailable-sections
  inventory, and fixed nonclaims state that the projection provides no
  Component Model runtime or packaging, no host loading or lifecycle
  management, no versioning negotiation, no resource-limit enforcement, and
  no hostile-plugin execution testing. Domain-separated SHA-256 digests
  authenticate the payload, source snapshot, identity, and every export
  signature; `verify_envelope` independently replays the exact bytes,
  counts, all closed sections, exclusion vocabulary, strict stable-id
  ordering, capability vocabulary, identity/version consistency, and every
  signature digest, so forged-but-re-signed mutations still fail closed.
  Options (`SPX-Q101`), out-of-vocabulary (`SPX-Q102`), budget
  (`SPX-Q103`), and consistency (`SPX-Q104`) diagnostics come from the
  previously unused `SPX-Q1xx` family (renamed from a transient `SPX-N1xx`
  choice that Unsafe Boundary Mechanics v1 already claimed). `tests/plugin_manifest_v1.rs` pins
  golden envelope KATs over two examples, determinism, every exclusion
  reason, independent digest recomputation, per-field tamper rejection
  including re-signed forgeries, budget exhaustion, source-drift fail-closed
  behavior through both embedded digests, CLI exit codes, and
  cross-consistency proving `required_capabilities` equals what `semaprax
  capability-manifest` derives for the same program and that listed exports
  carry byte-equal native symbols/signatures to what `semaprax abi-report`
  admits. The Plugins row moves from Missing to Partial.
- Added the locally evidenced Interface Package Report v1 tranche, the first
  executable slice of the completion-matrix row "Interface-first packages and
  target matrices". The new read-only `semaprax package-report <file>
  [--max-bytes N]` command and `package_report` library API project one
  verified module into one deterministic canonical
  `semaprax.package-report.v1` envelope: an interface-first package
  descriptor whose sorted admitted export inventory lists every explicit-ID
  monomorphic effect-free by-value `i64`/`bool` function with its interface
  parameter/result types, canonically rendered requires/ensures clauses,
  declared effects, persistent stable ID, and the exact Native64 prototype
  line extracted verbatim from the production native C11 projection under a
  per-export domain-separated digest; every other function is recorded under
  one of six closed exclusion reasons mirroring Canonical ABI Report v1. A
  fixed target availability matrix marks exactly `native64` and `wasm32`
  available for this profile, and an explicit closed ten-entry
  unavailable-capability inventory plus fixed nonclaims state that the report
  provides no resolver, lockfile, dependency model, package registry,
  version-compatibility engine, conformance tests, provenance, signatures,
  licenses, or SBOM. Domain-separated SHA-256 digests authenticate the
  payload and source snapshot; `verify_envelope` independently replays the
  exact bytes, package counts, both closed sections, exclusion vocabulary,
  strict stable-id ordering, and every export-signature digest, so
  forged-but-re-signed mutations still fail closed. Admission-profile,
  options (`SPX-P301`), budget (`SPX-P302`), and consistency (`SPX-P303`)
  diagnostics come from the previously unused `SPX-P3xx` family.
  `tests/package_report_v1.rs` pins golden envelope KATs over two examples,
  determinism, every exclusion reason, independent digest recomputation,
  per-field tamper rejection including re-signed forgeries, budget
  exhaustion, source-drift fail-closed behavior, CLI exit codes, and
  cross-consistency proving the listed exports equal what `semaprax
  abi-report` admits (with byte-equal native prototypes) and what `semaprax
  openapi` publishes for the same program. The Interface-first packages and
  target matrices row moves from Missing to Partial.
- Added the locally evidenced UI Dialect Schema Projection v1 tranche, the
  first executable slice of the completion-matrix row "First-class
  application/state/UI dialect". The new read-only `semaprax ui-schema <file>
  [--max-bytes N]` command and `ui_schema` library API project one verified
  module into a deterministic canonical `semaprax.ui-dialect-schema.v1`
  envelope describing its typed application schema: every public non-generic
  scalar-field record becomes one state-shape descriptor whose field names,
  `i64`/`bool` types, and offsets/sizes/alignments come exclusively from the
  checked Native64 compiler layouts, and every explicit-ID monomorphic
  by-value effect-free scalar function becomes one typed action descriptor
  with its parameter/result types under the exact Canonical ABI Report v1
  admission profile. Records outside the profile are excluded with the closed
  reasons `automatic_identity`, `generic_type`, `resource_type`,
  `variant_type`, or `mixed_field_types`, and functions reuse the six shared
  abi-report reasons; an explicit empty-by-default controls/accessibility/
  navigation section is always present as a reserved nonclaim field.
  Domain-separated SHA-256 digests authenticate the payload, source snapshot,
  every state-shape layout, and every action signature;
  `verify_envelope` replays all of them independently by rebuilding the
  canonical bytes from parsed values, so even consistently re-minted forgeries
  fail closed (`SPX-U101`-`SPX-U103`). Pinned golden envelope KATs over three
  examples, checked-layout cross-consistency including the Point record's
  exact offsets and padded 24/8 shape, action cross-consistency against
  abi-report signatures for the same program, determinism double runs, every
  exclusion reason, budget exhaustion, per-digest-field tamper rejection, and
  CLI exit codes are green locally in `tests/ui_schema_v1.rs`. The projection
  claims no rendering, runtime, DOM, typed update/view language constructs,
  semantic controls, accessibility, navigation, localization, assets, platform
  blocks, custom rendering, or target execution. The First-class
  application/state/UI dialect row moves from Missing to Partial.
- Added the locally evidenced Freestanding Object Profile v1 tranche, the
  first executable slice of the completion-matrix row "Embedded and
  real-time". The new read-only `semaprax freestanding-object <file>
  [--max-bytes N]` command and `freestanding_object` library API admit one
  verified effect-free scalar module (whole-module scalar gate, fail-closed)
  and emit one deterministic canonical `semaprax.freestanding.v1` envelope
  containing the complete freestanding C11 translation unit whose bytes start
  from the production native C11 projection with the host entry wrapper,
  `<stdio.h>`/`<stdlib.h>` includes, and `spx_public_failure` reporter
  excluded, plus two recorded substitutions: a closed failstop replacement
  for the hosted stderr/abort invariant reporter and external linkage for
  each module function so the relocatable object exports callable symbols.
  Four profile assertions — no-runtime, no-allocation, no-blocking, and
  no-libc-dependency — are computed by explicit deterministic textual checks,
  re-checked during independent envelope replay, and backed by a real
  toolchain gate in `tests/freestanding_object_v1.rs`: the emitted bytes are
  compiled twice with `-ffreestanding -nostdlib -c` into byte-identical
  relocatable objects whose `nm` surface must stay inside the declared
  allowed set (`memcpy`, `strcmp`, each with documented justification) while
  every module symbol remains externally defined; compiler discovery follows
  the existing native lanes and skips with an explicit message when no
  toolchain exists. Domain-separated SHA-256 digests authenticate the payload,
  source snapshot, and embedded unit; pinned golden KATs cover the envelope
  and translation-unit bytes, per-digest-field tamper rejection including
  forged-but-re-signed payloads caught by assertion replay, determinism,
  budget exhaustion, every admission rejection reason, and CLI exit codes.
  Option (`SPX-A101`), module-admission (`SPX-A102`), budget (`SPX-A103`),
  and consistency (`SPX-A104`) diagnostics fail closed from the previously
  unused `SPX-A1xx` family. The tranche claims no MMIO/volatile/atomics
  support, no linker-script control, no hardware/emulator execution, no
  interrupt or RTOS model, and no board targets; the artifact is a
  relocatable object for one effect-free scalar profile only, the command
  invokes no toolchain, and no completion beyond this bounded Partial slice
  is claimed. The Embedded and real-time row moves from Missing to Partial.
- Added Explicit Mutation v1, a bounded end-to-end language slice that moves
  the completion-matrix row "Immutable-by-default values and explicit
  mutation" from Missing to Partial. Local bindings may declare `let mut`,
  and simple locals admit a new statement-only assignment form
  `<binding> = <expr>;` over checked Copy scalar values (`i64`, `i32`, `u8`,
  `char`, `f32`, `f64`, `bool`) with exact type matching: the assigned value
  evaluates fully — checked arithmetic failure statuses propagate exactly as
  from an initializer — before one atomic store into the binding's existing
  value identity, so no new value id exists and evaluation stays
  left-to-right. Immutable by default is enforced everywhere else through the
  previously unused `SPX-U1xx` diagnostic family: assignment to an immutable
  binding (`SPX-U101`), exact type mismatch (`SPX-U102`), `mut` outside local
  `let` bindings such as parameters (`SPX-U103`), duplicate `mut` modifiers
  (`SPX-U104`), non-scalar/non-Copy targets or values (`SPX-U105`), and
  assignments inside contract expressions (`SPX-U106`); unknown targets keep
  the established unknown-value diagnostics. The grammar admits no assignment
  expression and no compound operators, so `(x = 2)` and chained `x = y = z`
  fail at parse time. The canonical formatter renders both forms with exact
  byte budgets and formats mutation-free programs byte-for-byte identically;
  HIR gains `ResolvedStatement::Assign` carrying the reused target binding
  plus per-binding mutability in resolver/validator scopes with matching
  iterative and recursive oracle paths; Graph JSON adds `"mutable":true` only
  on mutable lets and an additive `"kind":"assign"` node without touching
  schema selection (v10-v14) or any pinned non-mutation graph bytes; cleanup
  lowering treats assignments as initializer-equivalent value steps so
  straight-line mutation produces structurally identical CleanupPlan v2
  output (no slots, no finalizers); native C11 lowers to plain locals and
  plain C11 stores verified at O0/O2 including assigned-overflow failure
  statuses, and the Wasm core lane stores via `local.set` with identical
  Node-executed results and i32 overflow trapping. Evidence lives in
  `tests/explicit_mutation_v1.rs` (16 tests) and
  `examples/explicit_mutation.spx` under the example check/fmt gates.
  Non-claims: no field/aggregate mutation, no collection mutation, no
  reference/mutable-borrow semantics, no concurrency/memory-model rules, and
  no cross-task mutation.

- Added the locally evidenced Canonical ABI Report v1 tranche, the first
  executable slice of the completion-matrix row "Portable canonical ABI and
  native fast ABI". The new read-only `semaprax abi-report <file> --function
  ...` command and `abi_report` library API emit one deterministic canonical
  `semaprax.abi-report.v1` envelope describing, per explicitly selected
  explicit-ID monomorphic by-value `i64`/`bool` function, both the Native64
  fast ABI — the exact prototype line extracted verbatim from the production
  native C11 projection, sizes and alignments taken from the checked compiler
  layouts (`i64` 8/8, `bool` 1/1), by-value copy semantics, and the
  status/out-parameter contract — and the portable canonical mapping used by
  the Public Scalar Export Profile v1 Core-Wasm lane (Core-Wasm `i64`/`i32`
  signatures, injective raw export symbols, canonical bool boundary
  normalization identical to the emitted web-v4 adapters, and fixed copy-only
  behavior). Domain-separated SHA-256 digests authenticate the payload,
  source snapshot, every native prototype, and every rebuilt canonical
  object; `verify_envelope` replays all of them independently. Admission
  mirrors C Header Emission v1 with the same six closed exclusion reasons;
  overflow (`SPX-A203`), selection (`SPX-A202`), option (`SPX-A201`), and
  envelope/backend consistency (`SPX-A204`) diagnostics fail closed from the
  previously unused `SPX-A2xx` family. Pinned golden envelope KATs over two
  examples, byte-level cross-consistency against both real backend
  projections (native prototypes and wasmparser-decoded scalar-export module
  signatures), checked-layout agreement including the Native64/Wasm32 bool
  divergence, every exclusion reason, CLI exit codes, budget exhaustion, and
  per-digest-field tamper rejection are green locally in
  `tests/abi_report_v1.rs`. The report claims no interface semantics beyond
  selected scalar exports, no borrowing (copy-only slice), no cross-language
  conformance suites, no target execution, and no hosted promotion. The
  Portable canonical ABI and native fast ABI row moves from Missing to
  Partial.

- Added the locally evidenced Build Capability Manifest v1 tranche, the first
  executable slice of the completion-matrix row "Sandboxed builds and
  dependencies". The new read-only `semaprax capability-manifest <file>`
  command and `capability_manifest` library API project one verified module
  into a deterministic canonical `semaprax.capability-manifest.v1` envelope
  declaring its exact build capabilities: the sorted module permit inventory,
  every declared per-function effect set, every declared interface-import
  effect set, and an explicit empty-by-default ambient authority assertion
  over the closed five-domain vocabulary filesystem, home, network, process,
  and secrets. Every capability token anywhere in the module must sit inside
  that vocabulary or the command fails closed with the dedicated `SPX-K202`;
  interface permits no import consumes are checked but do not by themselves
  mark a domain as declared. Envelopes bind source snapshot digest, graph
  revision, module accounting, and the ambient section behind domain-separated
  SHA-256 digests; `verify_envelope` independently replays shape, byte count,
  digest, vocabulary, and ambient derivation, and
  `verify_envelope_against_source` fails closed on source drift. Output-budget
  overflow (`SPX-K203`) and malformed options (`SPX-K201`) also fail closed
  without truncation. Pinned golden envelope KATs over the effect-free example
  (all five domains `"none"`), exact declared-effect inventories, determinism,
  undeclared-capability injection rejected through both digest authentication
  and a consistent re-mint replay, out-of-vocabulary forgery rejection, tamper
  rejection, drift detection, budget exhaustion, and CLI exit codes are green
  locally in `tests/capability_manifest_v1.rs`. No sandbox is enforced at
  build time, no dependency resolution, lockfile, package registry, or
  network/home/secrets/filesystem/process enforcement machinery exists, and no
  target execution is claimed; enforcement against a declared manifest remains
  future work. The Sandboxed builds and dependencies row moves from Missing to
  Partial; current totals are 44 Partial/12 Missing.

- Added the locally evidenced C++ Shim Projection v1 tranche, the first
  executable slice of the completion-matrix row "C++". The new read-only
  `semaprax cxx-shim <file> --function ...` command and `cxx_shim` library
  API project explicitly selected explicit-ID monomorphic by-value
  `i64`/`bool` functions of one verified module into a deterministic
  C++17-compatible header fragment: an `extern "C"` block whose declaration
  lines are extracted verbatim from the production native C11 projection, so
  shim declarations always match the emitted ABI and cannot be silently name-
  mangled; generated comments carry only typed stable-ID, canonical contract,
  effect, status-contract, and by-value ownership facts under a fail-closed
  hygiene guard (`SPX-X104`); include guards derive only from sorted admitted
  stable identities and stay byte-stable across formatting-only edits and
  display-name-only renames while changing on identity renames. Canonical
  compact `semaprax.cxx-shim.v1` envelopes bind source snapshot digest, graph
  revision, selection/admission accounting, per-declaration digests, and the
  embedded fragment behind domain-separated SHA-256 digests with an
  independent `verify_envelope` replay that separately rejects tampering of
  every digest field, including forged-but-re-signed envelopes only the inner
  fragment replay catches. The admission profile is exactly C Header Emission
  v1's closed exclusion-reason set; output-budget overflow fails closed
  without truncation (`SPX-X103`), unknown or duplicate selections are hard
  errors (`SPX-X102`), all-excluded selections still yield a valid empty
  `extern "C"` block, and at least one admitted function requires the native
  lane to succeed. Pinned golden envelope/fragment KATs, native
  cross-consistency, byte-identical double runs, guard stability rules, and
  CLI exit codes are green locally in `tests/cxx_shim_projection_v1.rs`; no
  C++ compiler runs, no hosted promotion exists, and no header import or
  parsing, C++ compilation or conformance claim, exception/lifetime policy
  beyond the bounded slice, string/buffer/aggregate/resource mapping, adapter
   generation, or execution claim is made. The C++ interoperability row moves
   from Missing to Partial (current totals 45 Partial/11 Missing).
- Moved the workspace minimum supported Rust from 1.85 to 1.88 so private
  crates can track current dependencies, and bumped the exact-pinned private
  `semaprax-native-loader` dependency from `libloading 0.8.9` to `=0.9.0`.
  The three dynamic settlement symbol lookups pass `&[u8]` slices because
  libloading 0.9 replaced its lookup bound with `AsSymbolName`. The manifest
  MSRV fields, the README badge and install floor, the mandatory CI minimum-
  version job (`Rust 1.88 minimum`), and the corresponding quality-gate text
  moved together. The two hosted `ReactiveCircus/android-emulator-runner`
  pins were also re-pinned to the current `v2.37.0` tag commit
  `e89f39f1abbbd05b1113a29cf4db69e7540cae5a` per the Dependabot GitHub
  Actions bump. The hosted Android Emulator, JNI/APK, iOS Simulator, and
  sanitizer jobs must re-green on this commit before any new physical-runtime
  claim is counted, and this adds no completion transition.

- Added the locally evidenced C Header Emission v1 tranche, the first
  executable slice of the completion-matrix row "C and Objective-C". The new
  read-only `semaprax c-header <file> --function ...` command and
  `c_header` library API derive one deterministic C11 header for explicitly
  selected explicit-ID monomorphic by-value `i64`/`bool` functions. Emitted
  declaration lines are extracted verbatim from the production native C11
  projection, so header declarations always match the emitted ABI; generated
  comments carry only typed stable-ID, canonical contract, effect,
  status-contract, and by-value ownership facts under a fail-closed hygiene
  guard (`SPX-D104`); include guards derive only from sorted admitted stable
  identities and stay byte-stable across formatting-only edits and
  display-name-only renames while changing on identity renames. Canonical
  compact `semaprax.c-header.v1` envelopes bind source snapshot digest,
  graph revision, selection/admission accounting, per-declaration digests,
  and the embedded header behind domain-separated SHA-256 digests with an
  independent `verify_envelope` replay; output-budget overflow fails closed
  without truncation (`SPX-D103`), unknown or duplicate selections are hard
  errors (`SPX-D102`), all-excluded selections still yield a valid empty
  header, and at least one admitted function requires the native lane to
  succeed. Pinned golden envelope/header KATs, native cross-consistency,
  every exclusion reason, guard stability rules, tamper rejection, and CLI
  exit codes are green locally in `tests/c_header_emission_v1.rs`; no C
  compiler runs, no hosted promotion exists yet, and no header import, raw
  binding import, safe wrapper, Objective-C mapping, string/buffer mapping,
   or execution claim is made. The C and Objective-C interoperability row
   moves from Missing to Partial; the shared completion-matrix dashboard
   totals lines stay untouched pending hosted promotion.
- Added the locally evidenced Typed Hygienic Generation v1 tranche. The new
  read-only `semaprax hygienic-gen <file>` command and `hygienic` library API
  synthesize default constructors and scalar field accessors for admitted
  non-generic scalar records as typed AST nodes whose tails are real
  `ConstructRecord`/`Project` expressions, verify the combined program with
  the ordinary verifier, and project it through the real Graph module so every
  generated declaration has a resolved graph identity. Derived `__gen_` names
  are pure functions of the record's persistent stable ID (rename-with-same-id
  keeps them; movement changes nothing); collisions with existing symbols,
  prefix preemption (`SPX-Y102`/`SPX-Y103`), combined-program rejection
  (`SPX-Y104`), envelope floor exhaustion (`SPX-Y105`), and unresolved
  identities (`SPX-Y106`) all fail closed. Reports are canonical compact
  `semaprax.hygienic-gen.v1` JSON with an authenticated outer digest, closed
  exclusion reasons, byte-budget prefix truncation with stable order, and
  fixed nonclaims. No textual rewriting, macro system, cross-file scope,
  persistence, or target execution is claimed.
- Reorganized the documentation around a published book without moving any
  document path. Added `book.toml` and `docs/SUMMARY.md` (pinned mdBook
  0.5.4), so the existing flat `docs/*.md` files render as one searchable
  site grouped into Foundations; Status and process; Agent interface;
  Semantic workspace; Targets and backends; and Platform adapters, with a new
  `docs/index.md` portal mapping every document to a one-line purpose plus
  role-based entry points. De-duplicated prose rather than deleting content:
  the repository module map now lives only in `docs/ARCHITECTURE.md`
  ("Repository module map") with `AGENTS.md` linking to it and its
  conditional reads compressed into one topic table, and the README status
  section now points at the spec status headers that own the hosted-green
  evidence, which also fixes the garbled duplicated Agent Runtime sentence
  there. Added `.github/workflows/docs.yml`: it builds the book on every
  push and pull request with pinned mdBook and SHA-pinned official Pages
  actions, and deploys the HTML to GitHub Pages on main. This changes no
  language, protocol, backend, or completion-matrix claim;
  `tests/documentation.rs` continues to require every local markdown link to
  resolve.

- Added the locally evidenced checked `i32` scalar tranche: `i32` is now a
  Copy value type end-to-end. The change spans the lexer (explicit `42i32`
  suffix literals with stable `SPX-P003` range/suffix diagnostics; unsuffixed
  literals stay `i64` with no cross-width inference), parser, canonical
  formatter (the explicit suffix keeps the declared width round-trip stable),
  source verifier (`+`, `-`, `*`, `/` stay `i32` and are checked like `i64`;
  `%` stays restricted to `i64` via the existing `SPX-T208`), resolved HIR,
  Native64/Wasm32 aggregate and variant layouts (4/4 bytes), cleanup plans,
  Graph JSON (`"kind":"int32"` nodes carrying the exact value and
  `"kind":"primitive","name":"i32"` types), the strict native C11 backend
  (`int32_t` representation; arithmetic computes in `int64_t` and selects the
  same `SPX_STATUS_ARITHMETIC_*` codes as `i64`, including `INT32_MIN`
  negation and division overflow), and both Wasm core backends (`i32`
  valtype with signed ordering opcodes). The aggregate Wasm lane detects add/
  sub/mul overflow branchlessly via sign bits or i64 widening and selects the
  aggregate `STATUS_*` failure codes through `fail_if`; the core lane lowers
  checked i32 arithmetic inline on widened operands without new host imports
  and traps on detected overflow because that lane has no status plumbing.
  Ordered comparison is signed scalar ordering; equality follows the existing
  same-type rule; mixed-width operands are rejected by the verifier
  (`SPX-T208`, `SPX-T207`). i32-bearing records, record update, projection,
  params, returns, locals, calls, branches, cleanup plans, conformance traces
  (`{"kind":"int32","value":N}`), and contracts execute identically through
  native C11 at `-O0`/`-O2` and Node/Wasm (`tests/i32_scalars.rs`,
  `examples/integers_i32.spx`; an overflow probe asserts a non-success
  status natively and a trap under Node rather than silent wrapping).
  Deliberately out of scope for this tranche: generic instantiation arguments
  and generic template signature slots remain direct `i64`/`bool` only,
  Public Scalar Export Profile v1 remains i64/bool-only, the native host
  callable corpus remains i64/bool/resource-only, the Native Rust interop
  boundary rejects i32 scalars, integer remainder stays `i64`-only, and no
  completion-matrix row status changes.
- Added the locally evidenced unsigned-byte `u8` tranche: `u8` is now a
  checked-arithmetic Copy scalar end-to-end. The change spans the lexer
  (integer literals with an exact `u8` suffix; unsuffixed digit runs stay
  `i64`, and out-of-range or malformed suffixes select stable `SPX-P003`),
  parser, canonical formatter (the explicit suffix keeps the declared width
  stable across round trips), source verifier, resolved HIR, Native64/Wasm32
  layouts (`u8` occupies one byte at Native64 and four bytes at Wasm32), cleanup
  plans, Graph JSON (`"kind":"uint8"` nodes with the exact value plus
  `"layout_key":"scalar:u8"`), conformance trace results
  (`{"kind":"uint8","value":N}`), the native C11 backend (`uint8_t`
  representation; checked arithmetic computes in `int64_t` and range-checks
  0..=255 into the matching `SPX_STATUS_ARITHMETIC_*` statuses), and both Wasm
  core backends (`i32` valtype with unsigned ordering opcodes; checked u8
  arithmetic uses inline unsigned range checks without new host imports —
  aggregate-lane failures select the same status codes as i64 while legacy-lane
  failures trap). `%` stays i64-only and unary negation stays rejected for u8
  (`SPX-T208`, `SPX-T206`). U8-bearing records, record update, projection,
  params, returns, locals, calls, branches, and contracts execute identically
  through native C11 at `-O0`/`-O2` and Node/Wasm over 4,096 re-entries
  (`tests/u8_scalars.rs`, `examples/bytes_u8.spx`). Deliberately out of scope:
  generic instantiation arguments and template signature slots remain direct
  `i64`/`bool` only, Public Scalar Export Profile v1 remains i64/bool-only,
  the native host callable corpus and the Rust-interop boundary reject u8
  fail-closed, string/heap types remain unimplemented, and no completion-matrix
  row status changes.
- Added the locally evidenced Unicode scalar `char` tranche: `char` is now a
  first-class Copy value type end-to-end. The change spans the lexer (single
  scalar char literals with `\n`, `\r`, `\t`, `\0`, `\\`, `\'`, and
  `\u{...}` escapes plus stable `SPX-P006`/`SPX-P007`/`SPX-P008` diagnostics),
  parser, canonical formatter (printable ASCII projects directly, named
  escapes are preserved, every other scalar projects as a lowercase
  `\u{...}` escape so revisions round-trip exactly), source verifier,
  resolved HIR (`SPX-H006` fail-closed rejection of non-scalar literal
  payloads), Native64/Wasm32 aggregate and variant layouts (4/4 bytes),
  cleanup plans, Graph JSON (`"kind":"char"` nodes carrying the exact scalar
  value plus the canonical display form), the strict native C11 backend
  (`uint32_t` representation with unsigned comparison semantics), and both
  Wasm core backends (`i32` valtype with unsigned ordering opcodes).
  Ordered comparison (`<`, `<=`, `>`, `>=`) compares Unicode scalar values;
  equality follows the existing same-type rule; char arithmetic and negation
  remain rejected by the verifier (`SPX-T208`, `SPX-T206`). Char-bearing
  records, record update, projection, params, returns, locals, calls,
  branches, and contracts execute identically through native C11 at `-O0`/
  `-O2` and Node/Wasm over 4,096 re-entries (`tests/character_scalars.rs`,
  `examples/chars.spx`). Deliberately out of scope for this tranche: generic
  instantiation arguments and generic template signature slots remain direct
  `i64`/`bool` only, Public Scalar Export Profile v1 remains i64/bool-only,
  the native host callable corpus remains i64/bool/resource-only, string/
  heap types remain unimplemented (no allocation model exists yet), and no
  completion-matrix row changes; totals remain 39 Partial/17 Missing.
- Added the locally evidenced OpenAPI Schema Generation v1 tranche. The new
  read-only `semaprax openapi <file> --function ...` command and `openapi`
  library API project admitted monomorphic scalar signatures of one verified
  module into a canonical OpenAPI 3.1 document wrapped in a
  `semaprax.openapi.v1` envelope with domain-separated source, document, and
  input-binding digests; per-operation request/result schemas preserve
  authored parameter order and the shared status component is emitted only
  when a signature can surface compiler-owned arithmetic or contract failure.
  The companion `semaprax openapi-compat <base.json> <candidate.json>` command
  authenticates both envelopes exactly (schema, structure, payload digest,
  outer digest) before classifying their difference into closed breaking,
  non-breaking, and informational finding families with a deterministic
  verdict and migration note. Overflow of either output budget fails closed
  with no truncated bytes. New stable diagnostics: SPX-OA101 through
  SPX-OA105. See `docs/OPENAPI-V1.md` for the admission model, document
  shape, compatibility semantics, and explicit non-claims (no Protobuf/gRPC,
  GraphQL, or SQL projections, no schema import parsing, no live conformance
  fixtures, no registry/server hosting, no target execution).
- Added the locally evidenced Property-Test Generation v1 tranche. The new
  read-only `semaprax properties <file>` command and `properties` library API
  generate deterministic boundary-lattice plus seeded candidates from admitted
  monomorphic scalar signatures, filter them through authored `requires`
  clauses, evaluate bodies and interprocedural admitted callees with checked
  arithmetic, lazy booleans, lexical bindings, and call-depth/step budgets,
  and report the first exact `ensures` counterexample (clause index, canonical
  text, full argument tuple, observed result) in canonical compact
  `semaprax.property-tests.v1` JSON with fixed key order, domain-separated
  source digest binding, closed deferral/runtime/truncation reason sets,
  byte-budget prefix truncation, and fixed nonclaims. Unsupported shapes defer
  fail-closed with stable reasons instead of approximating. No target is
  executed, no symbolic or SMT discharge is claimed, and completion totals
  remain 39 Partial/17 Missing.
- Added the locally evidenced Graph Agent Transport v1: `semaprax serve
  <file>` now runs a deterministic newline-delimited JSON-RPC 2.0 loop that
  binds exactly one checked program per session and answers a closed
  `protocol`/`graph`/`context`/`context_v2`/`ping`/`shutdown` method set.
  Responses are canonical hand-rolled JSON; `graph`, `context`, and
  `context_v2` embed byte-identical payloads from the unchanged Graph and
  Agent Context serializers. The request grammar is closed (exact member set,
  unsigned-integer-or-bounded-string ids, object params only, batch arrays
  rejected), notifications never respond, oversized frames fail closed and
  stop the session, and the session gains no ambient read/write/process/network
  authority beyond its one host-named source. This is the first bounded slice
  of the roadmap "persistent graph daemon and JSON-RPC agent transport" item:
  persistent indexed revisions, incremental resolution, multi-source sessions,
  and network transports remain open, and no completion-matrix row changes;
  totals remain 39 Partial/17 Missing.

- Added the locally evidenced Public Project Native Publication v1 tranche.
  `semaprax build --target native` over a `semaprax.toml` project now publishes
  the linked entry closure as one create-new executable through the unchanged
  shared Clang C11 lane and exactly the linked entry HIR that Web publication
  and internal lowering-equivalence evidence consume. Publication rechecks
  every held manifest/source input before and after the boundary, rejects an
  existing destination with `SPX-I307`, reports post-publication drift as
  `SPX-J103` while preserving the retained executable, keeps Web-package bytes
  and all single-file/Workspace/Patch evidence unchanged, and preserves
  published behavior across stable-ID display renames. Project `run`, a public
  project test command, hostile-window no-clobber native publication,
  cross-build executable byte determinism, and exact-head hosted promotion of
  the new lane remain held; completion totals remain 39 Partial/17 Missing.
- Added the locally evidenced IEEE-754 floating-point scalar tranche: `f32`
  and `f64` are now first-class Copy value types end-to-end. The change spans
  the lexer (decimal float literals with an optional `f32` suffix and
  deterministic `SPX-P003` diagnostics), parser, canonical formatter
  (shortest round-trip decimals; whole values keep `.0`; f32 keeps its
  explicit suffix so revisions are stable), source verifier, resolved HIR
  (exact bit-pattern literals, finite-value validation, `SPX-H006` fail-closed
  rejection of NaN/infinity before any backend), Native64/Wasm32 aggregate and
  variant layouts, cleanup plans, Graph JSON (`"kind":"float32"/"float64"`
  nodes with bit-exact hex payloads), the strict native C11 backend, and the
  Wasm core backends. Float arithmetic (`+`, `-`, `*`, `/`) is total IEEE-754:
  overflow, signed zero, and division by zero never select a failure status,
  unlike checked i64 arithmetic. Comparisons and equality produce `bool`;
  unary negation is total; `%` on floats is a stable `SPX-T208` diagnostic.
  Float-bearing records, nested records, record update, projection, params,
  returns, locals, calls, branches, and contracts execute identically through
  native C11 at `-O0`/`-O2` and Node/Wasm over 4,096 re-entries
  (`tests/floating_point_scalars.rs`, `examples/floats.spx`). Deliberately out
  of scope for this tranche: generic instantiation arguments and generic
  template signature slots remain direct `i64`/`bool` only, Public Scalar
  Export Profile v1 remains i64/bool-only (`SPX-W115`), the native host
  callable corpus remains i64/bool/resource-only, string/heap types remain
  unimplemented (no allocation model exists yet), and no completion-matrix row
  changes; totals remain 39 Partial/17 Missing.

- Added the locally evidenced Public Native Rust SDK v1 Phase C. The
  still-unpublished builder now exposes a bounded API that invokes unchanged
  private A+B and publishes an exact dependency-free nine-file Cargo package
  with stable-ID-derived scalar export/import methods, a safe public facade,
  the existing private unsafe FFI quarantine, a canonical
  `semaprax.native-rust-sdk.v1` manifest, and deterministic current-host static
  archive metadata. Archive creation uses one explicit held absolute tool,
  an 8 MiB pre-digest cap, a private nonce stage, closed platform member/header
  grammar, one byte-identical object, and create-new outer publication. The
  calculator and callback consumers compile and run locked/offline without a
  compiler dependency after generation. Aggregate/resource/string/pointer ABI,
  async/cross-thread use, registry/CLI publication, independent linker-index
  semantic reconstruction, Phase-C cumulative allocation proof, and exact-head
  hosted promotion remain held. Existing private A+B bytes and claims are
  unchanged; completion totals remain 39 Partial/17 Missing.

- Upgraded the pinned cryptographic and Wasm validation stack to `hmac 0.13.0`,
  `sha2 0.11.0`, and `wasmparser 0.256.0` across the root workspace and isolated
  Component runner. SHA-256 rendering remains byte-exact and allocation-bounded;
  Target Evidence now reports the actual validator version, with regenerated
  v1/v2/v3 Target Evidence and Patch Evidence v2 known answers. No completion
  status changes.

- Added [Project Manifest v1](docs/PROJECT-MANIFEST-V1.md), a locally
  evidenced bounded multi-file pure-scalar build input. Its exact canonical
  `semaprax.toml` names 2–16 source files, one entry, one test module, and
  1–32 stable Web exports. One invocation holds the exact manifest/source
  inputs, reuses Semantic Workspace Phase-A once in memory without creating a
  managed workspace, and links real stable-ID provider bodies into entry/test
  HIR closures consumed by internal native equivalence evidence and Web
  lowering. The public Project CLI publishes only the Web package, with the
  separate digest-bound `semaprax.web-project.v1` manifest. Types,
  interface declarations plus interface/native imports, `use type` edges,
  permits/effects, generics, dependencies, registries, discovery, and
  capabilities are excluded; explicit stable-ID `use function` provider edges
  remain the sole cross-file composition mechanism. Public native executable
  publication and project run/test commands are held. A post-publication final
  input drift reports `SPX-J103`: its complete digest-bound package remains for
  caller reconciliation and is never deleted automatically. Its exact-head
  Ubuntu/macOS/Windows and Chromium/TypeScript matrix is green at
  `d883ace579bfd86f723cdc6819224fde51f0677d` in [run
  32523952912](https://github.com/wavect/semaprax/actions/runs/32523952912).
  This makes no completion-matrix status change: totals
  remain 39 Partial/17 Missing.

- Reconciled Windows desktop packaging with serviced hosted-runner images by
  pinning the canonical Visual Studio 18 product line while retaining exact
  `vswhere`, MSVC, linker, SDK, and import-library identity checks, including
  the hosted-observed serviced `link.exe` identity.

- Added [Public Wasm Scalar Exports v1](docs/WASM-SCALAR-EXPORTS-V1.md).
  Repeated `--export <stable-id>` options select 1–32 explicit persistent
  monomorphic `i64`/`bool` functions from a completely scalar, effect-free
  program. Under a caller-exclusive parent/new-tree publication contract, the
  path-based fresh no-clobber package rejects symlink/reparse parents and
  children, uses create-new for every fixed artifact, rebinds parent/output
  identities, and immediately replays its exact inventory/bytes before
  success. Cleanup reauthenticates both identities and removes only exact-byte
  expected files. It contains only stable-ID-derived Wasm
  adapters, a canonical digest-bound `semaprax.web.v4` manifest, a
  digest-authenticating runtime, frozen JavaScript bindings, TypeScript
  declarations, and a calculator consumer. The facade performs exact
  BigInt/Boolean conversion and returns the closed eight-case arithmetic and
  pre/post-contract status results. Aggregate/resource/generic/import/effect
  shapes reject without fallback, and ordinary no-export web-v3 output remains
  the legacy lane. Local executable evidence covers bounded admission,
  deterministic artifacts, Node consumption, mutation rejection, and stable-ID
  rename preservation. A locked Chromium loopback calculator job is wired but
  awaits exact-head hosted-green evidence; exact TypeScript 5.8.3 compilation
  of the real generated declaration consumer is locally green and wired into
  that job. Components, npm publication, imports/capabilities,
  aggregates/resources, callbacks/async, multi-engine conformance, provenance,
  and production readiness. The JavaScript and TypeScript completion row moves
  from Missing to Partial; current totals are 39 Partial/17 Missing.

- Private Native Rust Interoperability v1 A+B design and implementation are
  exact-head hosted green at
  `50b96dccabe3b3dcbcdf38bab380f3eb8699184c` in [run
  32402944574](https://github.com/wavect/semaprax/actions/runs/32402944574).
  The additive `import rust fn` scalar profile resolves an
  explicitly configured absolute Rust launcher only to discover one bounded
  sysroot, independently holds the direct compiler at that sysroot, requires a
  fixed-point sysroot check, and admits Rust artifacts only from that held
  direct image. Phase B uses one pre-effect 12-use process arena whose Windows
  attribute storage is queried, bounded, reserved, and materialized exactly,
  prepared filesystem inventories/names, one fixed-capacity no-growth store for the four
  authenticated `rustc -vV` fields, allocation-free final comparison/publication,
  and fail-stop process/handle settlement. Private A now has named pre-HIR and
  post-HIR retained/scratch envelopes, iterative renderer/replayer traversal,
  exact persistent allocation transfers, and minimum-minus-one entry gates;
  prepared Phase-B target arguments admit current-host underscore components
  without opening other punctuation, and the Linux link plan freezes the
  target's native-static library tail. Windows now freezes a verified absolute
  MSVC linker into exact `-fuse-ld` arguments while keeping ambient `PATH` out
  of the isolated child environment. Local builder 100/100, platform-system
  24/24, platform 10/10, source-contract 6/6, strict-Clippy, formatting, and
  security gates are green; Windows directory authority now excludes mutable
  directory length while retaining full file identity and reparse rejection.
  The run is green across Ubuntu, macOS, Windows, Rust 1.85, the Linux
  sanitizer lane, and Windows runtime/capacity settlement. This promotes only
  private A+B evidence. Public C remains held because the builder/types remain
  crate-private, all three crates remain unpublished, and the root package has
  no Native Rust Interoperability API or CLI. Compiler sysroot/dynamic-library
  descendant provenance, callable v2/v3, loader/host, `SPX-B104`, and existing
  wires/KATs are unchanged.

- Added Bounded Native Agent Runtime v1 A+B proof and the additive C1 injected-
  host Rust API:
  canonical Profile/Task/Action/Trace/Evidence, deterministic routing, injected
  fake-host streaming, registered read-only typed tools, capability/effect
  checks, cooperative cancellation, cumulative budgets, and independent replay.
  C1 exposes opaque Agent/run/sink types and no CLI, provider transport, ambient authority, language or
  backend semantics, durable memory, wallet, payment, or signing surface.
  Public Agent Runtime v1 is hosted GREEN at 8cf29aff8d1be3ccf74c36bc8c837f0c666ca067 (run 31591039261, 12/12 jobs, private and public deterministic fake-host gates on Ubuntu, macOS, and Windows). Private Economic Agent v1 A+B is exact-head hosted green at fe75c38d898b71e3ed5c57411fb46d0dbd4fc34b in run 31611748969, including both Economic gates on Ubuntu, macOS, and Windows. Public Economic Agent v1 C is exact-head hosted green at 03f1f2736de23d03b298f265f93409de89a6be95 in run 31616168124 (12/12 jobs), including the private, process-termination, and public Economic gates on Ubuntu, macOS, and Windows. Totals remain 38 Partial/18 Missing.

All notable changes to SEMAPRAX are documented here.

## 0.2.0 — 2026-08-07

- Added [Semantic Workspace Operations v1](docs/SEMANTIC-WORKSPACE-OPERATIONS-V1.md),
  a bounded shared-lock read-only compiler for explicit stable-ID declaration
  and direct import-alias renames over existing managed paths. It consumes one
  retained base graph/AST-HIR sidecar, builds one candidate graph, and emits an
  exact existing Change-v1 replacements proposal plus canonical derivation
  wrapper. Digest KATs are
  `sha256:3c7bf340a5313907edcec41748063e8666793ee76b903bc4e691871a843544b5`,
  `sha256:5c7a67d42ef76b3a241c0dc98f3d8919a799d3745bb6ae54a1d0289a51ee3e86`,
  and
  `sha256:80df18fea48a663e25cca66e90c0842fa8146ed35ab2ee30f2659728509dd2b7`.
  Added an outer canonical Operations Evidence wrapper, exact shared-lock
  verification receipt, and exclusive fresh-replay immutable publication
  route. Change-v1 bytes remain unchanged; a Change-v1 Evidence document alone
  binds only derived Change bytes, not Operations intent. The exact
  `dfc04278c6ba9a7dd247d4cc4add3af91f55b936` matrix is hosted green in [run
  31570834457](https://github.com/wavect/semaprax/actions/runs/31570834457);
  all 12 jobs passed, including the Operations process-termination gate on
  Ubuntu, macOS, and Windows. Totals remain 38 Partial/18 Missing.

- Added [Semantic Workspace v1](docs/SEMANTIC-WORKSPACE-V1.md),
  [Workspace Semantic Graph v1](docs/WORKSPACE-SEMANTIC-GRAPH-V1.md),
  [Workspace Analysis v1](docs/WORKSPACE-ANALYSIS-V1.md), and
  [Semantic Workspace Change v1](docs/SEMANTIC-WORKSPACE-CHANGE-V1.md).
  The additive initializer authenticates 2–16 existing sources and resolves
  explicit direct function/type imports in one managed generation. The public
  graph projects an entry-provider closure while its budget authenticates full
  managed-set work; Context, Impact, and Review add bounded read-only analysis
  over six typed edge families. Change C1/C2/C3 binds a 2–16-file
  replacements-only proposal to full-graph delta artifacts, exact verification
  and application receipts, and fresh replay-before-write under the exclusive
  lock. Candidate publication is no-clobber, two final checks precede the sole
  `ACTIVE` pivot, and post-pivot uncertainty is `SPX-I212`. Receipts are not
  authorization tokens. Residue changes authenticated state, so old Evidence
  may fail `SPX-G187`; regenerated Evidence may exact-reuse the candidate
  without a strategy claim. Change document KATs are Preview
  `fbfba16e8c3a822b65e59b2a16e2f28393b6d9d9552bcc95fa1363e2599ff8fc`,
  Context `18a7990f5b3e1d6a7b06586930684f24787119b99c1e3981c83d92f46d2db117`,
  Impact `07c556a41f0ed1d6c25d48743f9550cb6a90eb6d1d8fe26c3ab274feac19284b`,
  Review `86ef97e76b6e4ae55d43995a3f537aa5f55b4326cf51a1cfe7fc4127d5054662`,
  Evidence `0c5393cb128adc8223a82b7181229cb2c18cb495d714949ccc2dfba07b4402b0`,
  verification receipt
  `564bdc6b50e475b68321787997aab2b4e96ad23397212e0efefe45b8895561c0`,
  and application receipt
  `2aeb79acfa7420fd57f82d8afa436658c265bf5c02808d13bd7b6acaa6957636`.
  Local public C3 is 10/10 and private authority evidence is 11/11. Exact-head
  hosted Ubuntu/macOS/Windows, MSRV, Component, and dependency-policy evidence
  remains pending. Real process termination proves OS lock release and old/new
  managed state on tested filesystems only, not power-loss durability. No
  status changes: 38 Partial/18 Missing.

- Added [Semantic Workspace Patch Evidence
  v1](docs/SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md). The canonical outer
  capsule binds one exact Workspace Patch/preview and, for every sorted changed
  path, independently rebuilt source/Graph/Patch/Review/seven-assessment/
  supporting-evidence facts plus a child Semantic Patch Evidence v1 digest.
  `verify-workspace-patch-evidence` emits an exact-replay receipt;
  `workspace-apply-with-evidence` acquires the exclusive permanent lock first
  and requires exact typed and byte replay before candidate generation or
  staging, then enters the unchanged sealed Workspace publication core. The
  capsule and receipt grant no authority and aggregate neither Target Evidence
  nor Evidence v2. Raw capsule/receipt SHA-256 KAT pairs are v1
  `d0f0ec9abde015cd84745d8d71b260736874b7cff8f172194d04e8ebe489c197` /
  `ee310a2f848dd034c20f727f011f30db46dfe478bbc1169467dec0d57c266ae1`,
  v2 `95b054e188a4721e03c08b94afe0963394fc0af16be42ef3bdec0990218eb9f6` /
  `da2440da67c87ec0ab873599c911fc78e816d02fcd12195532ce93817a15df0b`,
  v3 `3fc5dc57a01ce2a9d1110dfd66ec96e9def90b8bfd3e5d2328aa9d4a81da19e4` /
  `b05b0516508c7850b409b1b81dedfc51c708bfbe6e73c94db77a1aadce35f757`,
  and mixed v1/v2/v3
  `de764637af59c533feaba15dca373408cb50972f81afd3fde903f463550fde27` /
  `3538b97acc1626972b0242085c87059c51b64c2ba7412172bbc2c5118f2f63c1`.
  Local public generation/verification is 6/6, apply 5/5, hostile 2/2,
  module units 8/8, shared Workspace core 39/39, Workspace integration 12/12,
  root library 496/496, and preservation 107/107; full local gates and security
  are green. The exact `cda4892ee74100fd11c5161ad857d469ec5e5421`
  corrective matrix is hosted green in [run
  31491573287](https://github.com/wavect/semaprax/actions/runs/31491573287),
  with all 12 jobs passing, including [Dependency
  policy](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116816),
  [Ubuntu](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779117078),
  [macOS](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116941),
  [Windows](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779117130),
  [MSRV](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116811),
  and [Component](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116886).
  The intermediate exact `658b2f4dc6d69974cef553dbd4e6eaecafacdd63`
  documentation/count head [run
  31490049153](https://github.com/wavect/semaprax/actions/runs/31490049153)
  was nonqualifying and cancelled: its macOS early-error precedence test
  observed `SPX-I210` instead of the expected `SPX-G150`; Windows was
  concurrency-cancelled after that failure and reported no product failure.
  The exact `3e41b3a0318730fec41e7d75438414e93dafa313` predecessor [run
  31486578192](https://github.com/wavect/semaprax/actions/runs/31486578192)
  was nonqualifying at 10/12: its macOS test observed `SPX-I210` instead of the
  expected stale `SPX-G152` during snapshot-lock handoff, and its Windows
  lock-precedence fixture hit OS error 33 while reopening the locked `LOCK`
  file. The corrective head makes the owned-snapshot lock release explicit and
  avoids that fixture-only reopen; it changes no wire contract. This adds no
  completion transition: the dashboard remains 38 Partial/18 Missing, and unified
  cross-file semantics, repository analysis, target/test execution,
  provenance/approval, raw-tree materialization, recovery/GC, and durability
  remain open.

- Added [Semantic Workspace Transaction
  v1](docs/SEMANTIC-WORKSPACE-TRANSACTION-V1.md) and [ADR
  0002](docs/decisions/0002-managed-workspace-generations.md). The opt-in
  protocol authenticates 2–16 canonical pre-existing sources, serializes
  cooperating readers and writers through one permanent lock, publishes a
  complete immutable candidate generation, and atomically pivots only
  `ACTIVE`. It embeds unchanged admitted Patch v1/v2 and the sole canonical
  Patch v3 per file. Original source paths are never rewritten, so no raw-file,
  Git, editor, repository-Graph, cross-file semantic, recovery/GC, or power-loss
  durability claim follows. Exact initial-revision/snapshot/preview KATs are
  `sha256:9a7368825342cee138d02a8037248e9a41ed0479d4f7c32a21c7ee7141cf280c`,
  `3646097c9fb8c47bced51cf2c404b886755f657c73c57afb18d25282574f0b80`,
  and `a4f1a9467d535aada97e7f253cf51c0d2168b5557a5a400d11692ac6966776b4`;
  mixed-v1/v2/v3 snapshot/preview KATs are
  `dfd35db518d0a8d94b83702dd1d2760ce9340b5875e0960ac573f84474c223b5`
  and `3cbd8d22bc26069387ac8ebce72ca590f095cbaa193b04bdef041e4c06beced1`.
  Local integration 12/12, hostile 5/5, workspace units 37/37, library 482/482,
  full gates, preservation, and security are green. The exact
  `afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` matrix is hosted green in [run
  31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068),
  including [Ubuntu job
  93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613)
  and [Windows job
  93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611);
  all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
  strict Clippy and is not green evidence. The completion matrix remains 38
  Partial/18 Missing.

- Added [Semantic Target Evidence v1](docs/SEMANTIC-TARGET-EVIDENCE-V1.md) and
  [Semantic Patch Evidence v2](docs/SEMANTIC-PATCH-EVIDENCE-V2.md).
  `target-evidence` reports exact base/candidate Graph JSON, typed zero
  capability delta, production C11 source, and structurally validated Wasm core
  digests/lengths without executing a target or discovering tests. Additive
  Evidence-v2 generation, verification, and lock-first A0 application bind
  that report while preserving Evidence v1 and ordinary `patch`. Target is
  9/9, target units 4/4, Evidence-v2 8/8, and library 439/439 locally. The exact
  `fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted green in [run
  31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793),
  including [Ubuntu job
  93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631);
  all 12 jobs passed. This changes none of the 38 Partial/18 Missing statuses,
  grants no authority, and does not implement or replace multi-file work.

- Added [Semantic Patch Evidence v1](docs/SEMANTIC-PATCH-EVIDENCE-V1.md).
  Fixed-arity `patch-evidence` and `verify-patch-evidence` commands emit and
  independently replay exact bounded capsules for Patch v1/v2 and the sole
  canonical Patch v3 operation. The separate `patch-with-evidence` route
  acquires the unchanged A0 lock first, requires exact replay before staging,
  then commits through unchanged A0; ordinary `patch` remains unchanged.
  Exact capsule SHA-256 KATs are
  `03befad24157620b56138e84d4495b1973d141275ee728493d5fbe4f0f6f09aa`,
  `23742f9b8a323003237106d7a800cc8fb98f53a68bd72f5e0961cf47c63f7bba`, and
  `d682e08b125451af3ed49dce03a0814e83ca5e665224fc3bc7ab7b314827f62c`;
  receipt KATs are
  `1f2733743aaf2f9d2b9ad6bf2709a6867f169f596be01a9d53e92daecb8730a1`,
  `6d8b13b3f54277e66a1ee501e1e71d6fe959a2ebcdbaa158a7ece20dde054e48`, and
  `13a99674a4c014d9f7f315d8108c3e5c870dcac2c5950ff3035ca1a1c155361b`.
  A+B integration is 11/11 with 5/5 internal units; Phase C integration is
  16/16 with 11/11 hook/limit units; library 420/420, doctest 37/37, full
  preservation, and security gates are locally green. The exact
  `34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted green
  in [run
  31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
  including [Ubuntu job
  93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949);
  all 12 jobs passed. `e04c2c9` was the failed Rust 1.97 lint predecessor, not
  green evidence. This moves only Proof-carrying
  patches from Missing to Partial: capsules are not signatures, authenticated
  provenance, approval, target/test execution, general proofs, reusable
  authorization, multi-file transactions, or new Graph/Cleanup/runtime
  semantics.

- Added [Bounded Semantic Review v1](docs/SEMANTIC-REVIEW-V1.md), a fixed-arity
  read-only `review <file> <patch.spatch>` command with canonical
  `semaprax.semantic-review.v1` JSON. Patch v1/v2 reports embed complete,
  nontruncated Semantic Impact v1 evidence under fixed limits; the sole
  canonical Patch v3 `assign-function-id` report instead embeds the exact
  shared Diagnostic Repair identity rebase and no Impact object. Every report
  carries the seven fixed sections `behavior`, `api_identity`,
  `security_authority`, `memory_ownership`, `target_artifact`, `migration`, and
  `unsafe`, with one evidence-linked closed finding per authored operation.
  Exact Patch v1/v2/v3 whole-report SHA-256 KATs are
  `054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946`,
  `37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421`, and
  `081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544`.
  Local Review integration is 10/10, hook/limit units are 4/4, and library
  408/408, full workspace, release, doctest, rustdoc, strict Clippy, format,
  diff, preservation, and independent security gates are green. The exact
  `2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in
  [run 31423743369 attempt
  1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
  including [Ubuntu job
  93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170);
  all 12 jobs passed. This is not Agent Context, target/test execution, a
  public verifier or proof artifact, authenticated patch provenance, human
  approval policy/UI, A0 apply/commit authority, repository/multi-file review,
  or general capability/security/unsafe/ABI analysis. Only the Semantic human
  review completion row moves from Missing to Partial.

- Added [Bounded Diagnostic Repair v1 and Semantic Patch
  v3](docs/DIAGNOSTIC-REPAIR-V1.md). Read-only `repairs` discovery emits
  canonical `semaprax.diagnostic-repair.v1` JSON for one exact `SPX-S103`
  automatic-function target, and read-only `repair` instantiation emits
  canonical `semaprax.diagnostic-repair-preview.v1` JSON after independently
  proving the exact one-annotation HIR/normalized-Graph rebase. The operation
  is classified `breaking_identity_rebase`. Its embedded
  `semaprax.semantic-patch.v3` is exactly one canonical LF-terminated three-line
  `assign-function-id` operation; `patch` revalidates every selector, the
  closed scalar Graph-v10 repair domain, and the complete rebase before
  applying through unchanged A0. Impact v1 rejects every syntactically valid,
  canonical v3 as `SPX-G110` before semantic selector interpretation; malformed
  or noncanonical v3 remains `SPX-G101`. Impact retains its v1/v2 bytes. Frozen
  query, preview, and independently
  authored candidate-Graph SHA-256 KATs are
  `ef689fed2c742dea6cedb0b8ec3d449e5facd8748dd00cb8a8f2e6115be82075`,
  `ae779749b252e5d9661172dfebcd3317211b97310eed57a0a6b7a692be1053e4`,
  and `d255c0e88ff497436ca0737ffd139cf47c2c142cf1b4f2da071514c0515ad2b3`.
  Local Phase A integration is 13/13; the Phase B semantic integration corpus
  is 7/7; v3 A0 hook units are 4/4; aggregate v3 integration-plus-hook evidence
  is 9/9 (seven semantic cases plus two bounded-work integration hooks, not the
  separate 4/4 internal units); and the library suite is 404/404. Full preservation is green and
  security review is clean. The exact `dae957a` full matrix is hosted green in
  [run 31418476217 attempt
  1](https://github.com/wavect/semaprax/actions/runs/31418476217/attempts/1),
  including [Ubuntu job
  93553147265](https://github.com/wavect/semaprax/actions/runs/31418476217/job/93553147265);
  all 12 jobs passed. Typed holes, other diagnostics and declaration kinds, repair
  ranking/composition/automatic application, other v3 operations, authenticated
  patch provenance, Graph or CleanupPlan schema/version/semantic-shape
  widening, Graph v11-v14 repair admission, backend/runtime semantic changes,
  and general or multi-file repair remain nonclaims. The admitted breaking
  operation does change Graph-v10 revision/identity/callee/derived-ID content
  and may rebase identity-bearing CleanupPlan content.
  Function and structural call-site bounds run on the parsed AST before HIR.
  After the patch parses as v3, its A0 initial source read and both final
  rechecks are capped at 16 MiB; initial oversize and concurrent greater-than-
  16-MiB growth fail closed without replacing the source. V1/v2 reads are
  unchanged.

- Added [`semaprax.semantic-impact.v1`](docs/SEMANTIC-IMPACT-V1.md),
  a deterministic read-only preview for one Semantic Patch v1/v2 file. It
  reports exact operation/change provenance and source consumers, and computes
  a byte/node/depth-bounded reverse-call closure only for exact generic-call
  instance changes. Reports bind the source base/candidate revisions, Graph
  v10-v14 schema, patch schema, and a domain-separated digest of the exact
  processed patch bytes. Source identity/bytes/revision are rechecked before
  return; patch-path provenance remains trusted input. The canonical report
  SHA-256 KAT is
  `94bbe5dcfe02f4b80b12ba5c8faf0889ddf11a96598072e539490c71a09518e9`.
  Local focused integration and internal Impact/call-index suites are 12/12 and
  4/4.
  This is single-file call impact, not repository-wide, non-call, repair,
  ranking, or commit authority. Impact itself emits no review sections; the
  separate Review v1 layer embeds its complete nontruncated report. The exact `1b3731a` full
  hosted matrix is green in [run 31408654657 attempt
  2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2),
  including [Ubuntu job
  93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404).

- Added [`semaprax.agent-context.v2`](docs/AGENT-CONTEXT-V2.md) as an
  explicit-direction extension of the byte/node-bounded context query. V1
  remains the exact default when `--direction` is absent; `forward`, `reverse`,
  and `both` select deterministic breadth-first call traversal with global
  stable-ID ordering, minimum-depth direction provenance, and separate
  traversal and reference frontiers. Frozen SHA-256 KATs are forward
  `922404133444942ab86607772362098e0f5656add6bea607a890be2bcfe5b7c9`,
  reverse `9a2ebfe569926e67f436379cf2b5c96d510daadd11d0a295ed54903cb612627b`,
  and both `4ec8a62a17551e87dc301d08f0a09c6159445757bca6dd9920a7db4e3790ce17`.
  Local v2 and legacy-v1 gates are 8/8 and 8/8; the full hosted matrix is green
  in [run 31397881268, including Ubuntu job
  93485198327](https://github.com/wavect/semaprax/actions/runs/31397881268/job/93485198327).
  V2 remains a call-graph query and does not claim general reverse semantic
  edges, impact analysis, ranking, repository indexing, persistence, or a
  graph daemon.

- Added bounded Semantic Patch v2 with an explicit schema line, atomic
  persistent record/case-member and variant-case renames, exact generic-call
  type-argument replacement, pattern-shorthand binding preservation, and a
  mandatory selective post-HIR semantic-delta gate. Schema-less v1 behavior is
  retained. Graph remains v14 and CleanupPlan selection is unchanged. The
  patch file itself remains trusted input; A0 authenticates source/staging, not
  concurrent patch-path replacement. The focused Patch v2 suite is 9/9, and
  the exact `f95d243` full matrix is hosted green in [run 31401200449 attempt
  2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2),
  including [Ubuntu job
  93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044).
  The isolated runtime lock repair is included in that exact green head;
  Patch v2 also remains covered by the green [Wasmtime job
  93505622110](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622110).

- Hardened single-file semantic patch commits against lost updates and leaf
  substitution. Patch application authenticates a canonical regular source
  leaf, serializes cooperating writers with a create-new sibling lock, uses
  bounded create-new staging candidates, preserves source permissions, syncs
  staged bytes, and rechecks exact source identity/bytes/revision plus staging
  path/handle identity/bytes at both final commit boundaries. Unix uses exact
  device/inode identity. Windows holds same-file handles and compares volume
  plus the available 64-bit file index; it does not claim identity uniqueness
  on ReFS 128-bit or other hostile non-unique-index environments. Identity-
  aware cleanup never removes a foreign replacement. Focused internal commit-
  race/failure/path-swap tests are 5/5 and integration patch tests are 17/17;
  the full matrix is hosted green in [run 31396483313, including Windows job
  93481068538](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068538).
  This remains a single-file cooperative protocol: predictable sibling names
  permit collision or stale-lock denial of service, crashes may leave locks,
  the containing directory remains trusted against non-cooperating mutation in
  the final portable path-based rename window, and parent-directory sync,
  power-loss durability, multi-file commits, and general typed repair/impact
  are not claimed.
- Added default-off Private Source-Option Propagation Component v10 for WIT
  package `semaprax:private@0.8.0`, interface `option-propagation`, and world
  `semaprax-private-v10`. Its sole export maps the exact compiler-owned
  `Option<i64>` through postfix `?` to `Option<bool>` as
  `evaluate(input: option<s64>, divisor: s64) -> result<option<bool>, status>`.
  Exact SHA-256 KATs are source revision
  `98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72`,
  Graph v11
  `96083f90fab18c919a96cee48109e606e089159e109869a42bdf48831743d45d`,
  prelude v1
  `d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e`,
  `Option<i64>` layout
  `79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda`,
  `Option<bool>` layout
  `dec126293ece7ec0e48d3d85ccdb494f7c7cfe4c3d4a9b1a61b50f6f862ff038`,
  CleanupPlan v3
  `d07fa51fc6f192a43318140264fa0e5964933ed90bc065cc8c74708e258ff92f`,
  generated core
  `16d1d34024e3fad920d8d00a61d7cb3bd010335ca382f23615b3b3da4143aaec`,
  profile
  `f53a0c21638b5a360faa19ad4fdef68f6d861a5baffe39422847128686e82bef`,
  raw component
  `f5770bdfdbc862ea39640b2c706c1d9ea171164c220d18366e25b3219443ad0d`,
  and artifact DAG
  `90ab80260c84abfe85d1edc666ab3750b81388e6e4cffd7ca21c301b9d0ee589`.
  Typed and raw gates cover `Some`/`None`, contracts, checked arithmetic,
  sticky failure, status-first/tag-last publication, full poison, invalid
  input/output tags and booleans, unknown status, repeated and fresh instances,
  and out-of-band fuel exhaustion. Local core 5/5, component 4/4, CI-lock 4/4,
  full, hostile, and security gates are green; the zero-import pinned Rust
  1.97.1/Wasmtime 47 v3-v10 runner is hosted green in [run 31396483313, job
  93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502).
  V1-v9 bytes and KATs remain unchanged. This exact fixture does not establish
  general source selection/export, general `Result`/`Option`/`?` or algebraic
  Component mapping, nested/resource/non-Copy carriers, imports/capabilities,
  callbacks/async, callable/FFI or public ABI, browser/multi-engine
  conformance, package negotiation, or `SPX-B104`/`SPX-W111` widening.
- Added bounded explicitly instantiated generic Copy functions. One or two
  owner/index-stable parameters may appear directly in by-value scalar
  signatures, concrete calls must supply ordered `i64`/`bool` arguments, and
  templates remain effect-free and outside generic-to-generic call chains or
  recursion. Unused templates are verified over every direct-scalar
  substitution without being materialized; explicitly referenced instances
  receive exact domain-separated HIR identities, native symbols, and Wasm
  indices. Program-wide Graph v14 takes precedence over v13/v12/v11/v10 and
  serializes exact function-template, function-instance, and call-instance
  meaning. Corrected same-schema Graph v14 function-template serialization by
  adding the missing array delimiters around `type_parameters`; two-parameter
  templates previously produced invalid JSON in module, Agent Context, and
  bounded-context projections. Its migrated frozen SHA-256 KATs are module
  `7a61fa6229f2db7aca6a035fd961720e8a401c138cc66c9cd71c64d45bed5efd`,
  Agent Context
  `2841401e7ba85fa8e47b3c35a15ae401b4a271d2500d70bbf3627f1453869eb6`,
  and bounded context
  `d7bda2be1fc366195ffb00a9e20b2b03204b4dd6f46e8019842dd84f70b54ab8`.
  The corrected JSON parse regressions and KATs are locally green and hosted
  green in [run 31390043736, Ubuntu job
  93459346296](https://github.com/wavect/semaprax/actions/runs/31390043736/job/93459346296). Separate strict
  C11 O0/O2 and 4,096-entry Node/Wasm execution evidence plus its independent
  security review are green locally and were hosted in [run 31385406865,
  Ubuntu job
  93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338). CleanupPlan v2
  remains byte/schema/meaning compatible and template-ID-only; HIR and Graph
  authenticate the exact instance before replay. Inference, constraints,
  aggregate/resource/non-Copy signatures, effects, generic entrypoints,
  callable/resource admission, general/public Component mapping, and stable
  public ABI remain closed; the exact private v9 profile below is separately
  gated.
- Added default-off Private Generic-Function Instance Component v9 for exact
  WIT package `semaprax:private@0.7.0`, interface
  `generic-function-instances`, and world `semaprax-private-v9`. The three
  phantom Copy templates `preserve<T>`, `invert<T>`, and `ordered<T,U>`
  materialize exactly six explicitly referenced Graph-v14 function instances
  in frozen export order, each with the same
  `(marker: bool, control: s64) -> result<bool, status>` WIT signature and no
  authored record or layout roots. Admission selects exact
  `FunctionInstanceId`s, one monomorphic materializer, and `app.main`; it does
  not substitute declaration IDs or wrappers. Frozen SHA-256 KATs are source
  revision
  `218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c`,
  Graph v14
  `62907c4b95495bb573b2b37de9f0b08c7a82218934154521e8c0c8396158cc6e`,
  generated core
  `9f178207a0406f740198ee8c71d5d008efdf4d995ff04e11e80ea73b79155d44`,
  plan `edd11c98bbc902d9dbc9c942375477fcf1e6c3f1befbe3c4a9f260107104485e`,
  profile `365897ddb2770cc25a11690dddbfef5d232244ec5d328c79a24a1410e684615e`,
  raw component
  `3cf6c7d7d02e838fb374478a2b5b25077c7c612ad36e30deaffd15311a25a688`,
  and artifact DAG
  `2623ff9a7eda5526616a15befd4951de86874a59911dcba2a7d3bcc2d178a474`.
  Local core 5/5, component 4/4, CI-lock 4/4, full gates, and independent
  security review are green, including rejection of all 15 same-signature
  swaps (eight behaviorally observable and seven identity-only). The isolated
  zero-import, empty-linker, no-WASI Rust 1.97.1/Wasmtime 47 typed runner is
  hosted green in [run 31392541096, job
  93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492).
  V1-v8 bytes remain unchanged. This exact fixture
  opens no inference or constraints, general source selection/export,
  general generic-function Component mapping, aggregates/resources/non-Copy
  values, imports/capabilities, callbacks/async, callable/FFI or public ABI,
  browser/multi-engine conformance, package negotiation, or
  `SPX-B104`/`SPX-W111` gate.
- Added default-off Private Record-Pattern Projection Component v8 for exact
  WIT package `semaprax:private@0.6.0`, interface
  `record-pattern-projections`, world `semaprax-private-v8`, and four ordered
  monomorphic preserve/invert exports over the distinct same-layout
  `Phantom<i64>` and `Phantom<bool>` instances. It binds exact source, generated
  core, two Wasm32 layouts, Graph v13, projection plan, profile, component, and
  artifact DAG. Primary KATs are source revision
  `sha256:2baac0c0920dbb153789767bf506a4a81713081586a81444d8e5f5a8f5a8516d`,
  generated core
  `b6e1dbf9522dbb98df9b6fcd370b562a9a722fcc672d44488aed80f13b7ad39e`,
  profile `79d4bade38dd3fff9c7145b406bb0bb265ff3ef7cf084edac83384c84610bce2`,
  component bytes
  `d88590752ed7b08b0f0a32019ba8b4c5fc489d59f06b96986d7ad69e2554a10e`,
  and artifact DAG
  `e32fe0a15a3458f16aa4da59d87683013dbeba03754966f35e0cb63600e613a3`.
  Local exact/upstream validation, all six identity-swap rejections, four
  behaviorally observable polarity swaps, generated-core Node behavior,
  poison/invalid-value closure, source locks, strict gates, and independent
  security review are green. The isolated pinned Rust 1.97.1/Wasmtime 47 typed
  runner is hosted green in [run 31385406865, job
  93445428268](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428268). V1-v7
  bytes remain unchanged; v8 adds no generic-function component, general
  exporter, imports/capabilities/resources, public ABI, browser/multi-engine,
  package-negotiation, or `SPX-B104`/`SPX-W111` claim.

- Added bounded irrefutable Copy-record destructuring in `match`. Exact named
  fields may recursively destructure records, bind scalar or whole Copy-record
  values with shorthand or renamed bindings, or ignore fields; one top-level
  wildcard remains binding-free. Source/HIR reject missing, duplicate, foreign,
  resource/non-Copy, multi-arm, and non-scalar-result forms. Explicit record
  patterns select program-wide Graph v13 above v12/v11/v10 when no generic
  function declaration selects v14, and serialize exact
  concrete instances, stable field IDs, and binding identities; wildcard-only
  record matches preserve the prior schema. CleanupPlan v2/v3 remains unchanged
  and straight-line. Native C11 O0/O2 and Node/Wasm 4,096-entry evidence covers
  one-evaluation, nested/generic and whole-record bindings, bool paths, failure
  precedence, postconditions, and poison. The Ubuntu gate is hosted green in
  [run 31373317800, job
  93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130),
  and independent security review found no P0/P1.
  Refutable/literal/guard/or/rest/nested-variant patterns, non-Copy or
  resource matching, aggregate arm results, and public aggregate ABI admission
  remain closed.
- Added default-off Private Generic Record Component v7 for exact WIT package
  `semaprax:private@0.5.0`, interface `generic-records`, world
  `semaprax-private-v7`, and four exports over `Duo<i64, bool>`,
  `Duo<bool, i64>`, `Phantom<i64>`, and `Phantom<bool>`. The exact source,
  generated core, four concrete layouts, Graph-v12 and plan bindings, profile,
  component, ordered type arguments, and same-layout/distinct-instance Phantom
  mapping are authenticated. Local core/component hostility, upstream
  validation, Node core execution, default-consumer hiding, source locks,
  strict gates, and independent security review are green. The isolated pinned
  Rust 1.97.1/Wasmtime 47 typed runner is hosted green in [run 31373317800, job
  93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
  V1-v6 bytes remain unchanged; this adds no
  general source selection/exporter, nested/resource/non-Copy records,
  imports/capabilities/callbacks/async, callable/FFI or public ABI, browser/
  multi-engine conformance, package negotiation, or `SPX-B104`/`SPX-W111`
  widening.
- Added default-off Private Nested Record Component v6 for exact WIT package
  `semaprax:private@0.4.0`, interface `nested-records`, world
  `semaprax-private-v6`, and one `transform` export over fixed nested
  `inner`/`outer` scalar records inside the unchanged physical-status result.
  Source, generated core, both Wasm32 layouts, profile, component, and complete
  DAG have frozen KATs; local exact-profile/upstream validation, hostile
  mutation/cross-version closure, core execution, default-consumer hiding,
  source locks, strict Clippy, and independent security review are green. The
  isolated pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run
  31365363898, job
  93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974).
  V1-v5 bytes stay unchanged; this adds no general/empty/generic/resource
  records, variants/algebraic nesting, imports/capabilities/async, public ABI,
  browser/multi-engine support, package negotiation, or
  `SPX-B104`/`SPX-W111` widening.
- Added bounded explicitly instantiated generic Copy records with one or more
  owner/index-stable parameters, direct scalar/own-parameter template fields,
  and direct `i64`/`bool` concrete arguments. Canonical source, source
  verification, resolved HIR, construction/projection/immutable update,
  concrete type facts, exact-instance Native64/Wasm32 layout caches and
  digests, native C symbols, and Wasm lowering now agree for `Box<T>`,
  `Pair<T>`, and ordered `Duo<T, U>` instances. Generic-record programs select
  program-wide Graph v12 above the existing v11 Option/v10 legacy lattice;
  legacy graph snapshots remain byte-identical. Strict C11 O0/O2 and Node/Wasm
  re-entry cover construction, update, pass/return, both bool arms, sticky
  failure order, and poisoned outputs and are hosted green in [run 31365363898,
  Ubuntu job
  93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995).
  Generic-record inference and broader generic-function signatures,
  nested/resource/non-Copy arguments or fields, record patterns/matching,
  public aggregate/callable/FFI ABIs, and resource admission remain closed.
- Added default-off Private Scalar Algebraic Component v5 with six fixed WIT
  0.3 exports for `Option<i64>`, `Option<bool>`, and the complete direct-copy
  `Result<T, E>` matrix over `i64`/`bool`. Language carrier arms remain ordinary
  values nested inside the unchanged outer physical-status result. The exact
  capability-free source table/order, prelude layouts, stable-ID/core-index/WIT
  mapping, canonical memory, and source/core/profile/component DAG are
  authenticated without inferring identity from equal signatures or layouts.
  Core/component KATs, hostile mutation/reindexing/cross-version closure,
  upstream validation, invalid-value traps, default-consumer hiding, and the
  isolated pinned-Rust-1.97.1 typed Wasmtime matrix are hosted green in
  [run 31360176398, job 93367728269](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728269).
  V1-v4 bytes remain unchanged; this adds no
  general exporter, resources/non-copy carriers, imports/capabilities, async,
  public API/ABI, callable/FFI mapping, or `SPX-B104`/`SPX-W111` widening.
- Added default-off Private Source-Result Component v4 for one exact
  effect-free source closure. It compiles ordinary compiler-owned
  `Result<i64, bool>` plus postfix `?` into `Result<bool, bool>`, then lifts the
  result as the distinct WIT 0.2 type
  `result<result<bool, bool>, status>`. Source `Ok`/`Err` remain the inner
  language result while recognized compiler status is the outer error;
  invalid internal tags and unknown statuses trap. The artifact binds the
  exact source revision, generated core, prelude, both layout-v2 digests, and
  v4 profile with independent parsing, upstream validation, canonical-byte
  mutation closure, and local generated-core Node execution. The isolated
  Wasmtime 47.0.3 runner and CI source locks cover ten typed outcomes,
  same/fresh-instance calls, zero imports, empty linker/no WASI, and
  out-of-band fuel failure. The v4 runner is hosted green in [run 31356536123,
  job 93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).
  This does
  not alter v1-v3 artifacts, public component/aggregate ABI, resources,
  imports/capabilities, general `Result`/`Option`/`?`, callable/FFI signatures,
  or `SPX-B104`.
- Added bounded postfix `?` for ordinary compiler-owned direct-scalar Copy
  `Option<T>` into `Option<U>`. `Some` extracts the exact source payload;
  payload-free `None` reconstructs the exact outer instance as a normal
  status-zero result, skips later body work, and still traverses shared
  postconditions before success-only publication. CleanupPlan v3 authenticates
  the Option source, members, and source/target instances; Graph v11 emits
  `try_option`. Both schemas are feature-minimal: Result-only and
  propagation-free programs remain byte-compatible CleanupPlan v2/Graph v10,
  and agent context uses the program-wide graph schema even for a legacy root.
  Local strict C11 O0/O2 and Node/Wasm evidence covers `i64` to `bool`, `bool`
  to `i64`, `Some`, `None`, skipped failure, postcondition/physical-status
  separation, poison, invalid tags, and repeated Wasm entry. Hosted matrix
  evidence is pending. Nested/resource/non-copy arguments, residual conversion,
  `?` in contracts, public aggregate ABI, callable/component aggregate
  signatures, and public resource admission remain closed.
- Added bounded typed postfix `?` for ordinary compiler-owned
  `Result<T, E>` with direct `i64`/`bool` arguments. Source and HIR authenticate
  the exact prelude members, source and outer instances, one operand
  evaluation, exact `E`, normal-result `Err` staging, and a shared
  postcondition/publication epilogue. CleanupPlan v2 makes body-versus-residual
  Copy-result staging explicit and independently replayable; Graph v10 exposes
  the same evaluation-once and shared-epilogue meaning. Native C11 O0/O2 and
  real Node/Wasm cover different source/outer layouts, later-expression skip,
  physical-status separation, postcondition failures, poison, invalid tags,
  and re-entry. The conformance-trace protocol remains closed to aggregate
  values. Resource/nested arguments, generic-function use of `?`, broader
  non-Copy generic records, non-copy
  propagation, residual conversion, `?` in contracts, public aggregate ABI,
  callable/component aggregate signatures, and public resource admission are
  unchanged nonclaims.
- Hosted [run 31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690)
  is fully green for the typed-`?` tranche across Linux, macOS, Windows, MSRV,
  sanitizers, the isolated Wasmtime runner, and the private mobile/application
  jobs. The macOS provider export lock is bound to the Graph-v10-derived exact
  descriptor/execute/settle symbols and has a hostile source-lock regression.
- Hosted [run 31347109201](https://github.com/wavect/semaprax/actions/runs/31347109201)
  supersedes the pending generic/prelude and component-runner notes below. It
  is fully green across the configured matrix; the isolated prelude-bound
  Wasmtime runner is green in [job
  93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212).
- Superseding the earlier configured/pending notes below, hosted run
  [31338834586](https://github.com/wavect/semaprax/actions/runs/31338834586)
  is green for Portable Result Component v3
  ([job 93309086213](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086213)),
  the private macOS engine plus AppKit package/runtime
  ([job 93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230)),
  the private Swift/iOS app plus XCFramework
  ([job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228)),
  and the private Android JNI/Kotlin app
  ([job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206)).
  A later full-matrix [run 31343897595](https://github.com/wavect/semaprax/actions/runs/31343897595)
  is green, including the Windows engine and Win32 UI package/runtime
  ([job 93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480))
  and the bounded Copy Variants + Match v1 tests on Linux, macOS, and Windows.
- Superseding the earlier non-generic/Graph-v8 boundary below, copy variants now
  admit explicitly instantiated nominal templates with direct `i64`/`bool`
  arguments. Compiler-owned ordinary `Option<T>` and `Result<T, E>` are the
  same versioned generic-variant mechanism, not backend intrinsics. Graph v9
  authenticates owner/index-stable parameters, exact concrete arguments, and
  `semaprax.prelude.v1`; graph revision v2 binds canonical source plus the
  prelude contract. Internal Native64/Wasm32 layout digest v2 and distinct
  concrete-instance symbols prevent instance confusion. Native C11 O0/O2 and
  real Node/Wasm execute the generic and prelude cases with deterministic
  layout, poison, invalid-tag, and repeated-call evidence. That earlier tranche
  did not include generic functions or records, nested/resource arguments,
  non-copy matching, stable public
  aggregate ABI, and callable/component aggregate admission remain closed.
- Added bounded executable copy variants and exhaustive `match`: non-generic
  nominal unit/direct-`i64`/direct-`bool` cases, explicit construction,
  persistent case/payload identities, declaration-order `u32` tags, checked
  Native64/Wasm32 internal layouts, stable diagnostics, CleanupPlan v1
  variant-case replay, native C11 O0/O2 execution, and real Node/Wasm execution
  with selected-arm-only behavior, full poison, invalid-tag closure, and
  shadow-stack re-entry. Graph is now `semaprax.graph.v8`. Generic variants,
  `Option`, `Result`, `?`, resource-bearing payloads, non-copy ownership modes,
  stable public aggregate ABI, callable/component signatures, and public
  resource admission remain closed.
  The complete hosted matrix is green in
  [run 31343897595](https://github.com/wavect/semaprax/actions/runs/31343897595).
- Graph v7 added
  canonical immutable record update across source, resolved HIR, Graph, and
  context traversal. Update meaning is base-first with replacement expressions
  in authored order; v6/v7 and v7/v8 schema confusion reject as documented in
  [MIGRATIONS.md](docs/MIGRATIONS.md).
- Added checked deterministic Native64/Wasm32 layouts for nested record fields
  in the admitted `i64`/`bool`/direct-trivial-resource slice. Cleanup-plan
  construction and independent replay now cover partial initialization and
  immutable update, including untouched-field transfer and reverse exact-once
  cleanup of displaced live fields. Empty records are frozen to one byte with
  alignment one on both profiles.
- Added production-reachable native C11 O0/O2 and Node-executed browser Wasm
  lowering for nested scalar `i64`/`bool` records, including construction,
  projection, immutable update, base-first/authored replacement order,
  status/out poison, internal aggregate pointer parameters, caller-owned
  results, and Wasm shadow-stack re-entry. A separate private test-only harness
  projects one direct-trivial-resource record scenario from the same cleanup
  plan into C and real Wasm, proving an exact common finalization trace and zero
  liveness. This does not establish a stable public aggregate ABI, public
  resource-record execution, callable/component aggregate signatures, or any
  change to `SPX-B104`/`SPX-W111`.
- Added deterministic offline [agent context economics
  v1](docs/AGENT-ECONOMICS-V1.md): four checked maintenance questions, exact
  context/economics goldens, UTF-8 byte and node counts, an explicitly
  non-model lexical unit, manifest/context digests, exact scored label IDs,
  reviewed relevance/evidence recall, mutation and hostile path/facet gates,
  plus exact-case/separator-normal/Windows-forbidden-and-reserved-safe paths and
  explicit-or-unique-target-merge-base plus dirty-Git fail-closed
  `quick`/`changed`/`full` quality routing with a profile-exact
  ordered executable gate plan. The small corpus honestly records context
  larger than source; Graphify adoption and model-token savings remain
  unclaimed.
- Added [`semaprax.agent-context.v1`](docs/AGENT-CONTEXT-V1.md): the `context`
  CLI now emits deterministic whole-JSON byte- and function-node-bounded facts,
  exact used/omitted/deferred accounting, closed truncation reasons, strict
  options, and query-bound stable-ID progress frontiers with non-dangling
  emitted calls and permanent oversize rejection. Compact Graph-v8
  contracts, parameter/result ownership, effects, and reference-closed types
  are filterable; cleanup/lifecycle/import subgraphs are not claimed, and
  absent target/diagnostic/test graph facts are explicitly unavailable. This
  is not an exact model-token budget or repository-wide relevance/impact claim.
- Added the private [native desktop application
  v1](docs/DESKTOP-NATIVE-APP-V1.md): a feature-gated headless macOS `APPL`
  bundle and Windows portable PE application directory package one exact
  callable-v3 provider/descriptor with the existing loader and authenticated
  host. Local macOS execution proves two generation-rotating owned calls and
  exact replay. The Windows package/runtime path and hosted desktop executions
  remain configured and pending; UI, accessibility, lifecycle,
  installer/signing, public admission, and `SPX-B104` stay closed.
- Added private [native desktop UI v1](docs/DESKTOP-NATIVE-UI-V1.md): exact
  AppKit and Win32 frontends compose the package-bound desktop engine with one visible
  native window/button, native accessibility-name query, delayed control event,
  event-loop close/termination, pre-launch SHA-256 engine-byte verification,
  bounded AppKit termination, exact Windows imports/no export directory,
  double-build artifact inspection, and closed packaging. Source locks and local AppKit compilation are green; hosted
  packaged macOS/Windows execution is configured and pending. This adds no
  SEMAPRAX UI syntax, SwiftUI/WinUI, general accessibility/lifecycle,
  distribution, public admission, or `SPX-B104` claim.
- Added the private [Apple Swift ownership adapter
  v1](docs/APPLE-SWIFT-OWNERSHIP-V1.md): a feature-gated same-thread Rust host,
  generation-tagged Swift wrapper, target-bound device/simulator providers,
  private XCFramework packager, and installed arm64-Simulator app gate. Local
  Rust/source-lock evidence and the bounded hosted Apple path are green in
  [run 31333469714, job
  93295293995](https://github.com/wavect/semaprax/actions/runs/31333469714/job/93295293995).
  Public framework, device, UI/lifecycle, admission, and `SPX-B104` claims stay
  closed.
- Added the private [WIT boundary
  v1](docs/WIT-COMPONENT-BOUNDARY-V1.md): deterministic `SPXWIT01`
  WIT/schema/JavaScript bytes, a frozen digest, mutation rejection, exact
  status bounds, and Node adapter execution. A separate standards-valid scalar
  component binary now has a frozen digest, an independent exact-profile
  parser, hostile mutation coverage, default-surface closure, and private Node
  execution of its extracted core module. Checked component v2 additionally
  composes the exact SEMAPRAX-generated scalar core with a frozen checked
  runtime, exposes read-only artifact digests, passes pinned upstream
  `wasmparser` validation plus rehashed signature/body/cardinality/lift
  hostiles, and executes generated success, overflow, and contract failure
  through authenticated private `evaluate()`. Portable Result Component v3
  adds the exact private `result<s64, status>` lift, independent parser and
  upstream validation, poison/sticky-status evidence, and local typed Wasmtime
  execution of success, addition overflow, division by zero, precondition, and
  postcondition outcomes with zero imports, an empty linker, and no WASI. Its
  dependency/MSRV graph is isolated; hosted Wasmtime is configured and pending.
  Source-language `Result`/`Option`, records/resources/imports/async,
  capabilities, multi-engine/browser conformance, public API, and `SPX-B104`
  remain closed.
- Added a mandatory private Android Emulator runtime gate. A hidden closed
  selector emits exact arm64 and x86_64-emulator dynamic descriptors
  and Bionic/ELF-guarded strict-C providers; the unpublished host now compiles
  for Android and connects the real dynamic loader to the unchanged receipt
  ledger. The pinned NDK/API-35 lane compiles both architectures and
  executes `token.discard-two` at O0/O2 in an x86_64 emulator
  with exact finalizers and zero measured Rust allocation across the
  irreversible interval. [Run 31320436726, job
  93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
  is green. That standalone-process lane proves no JNI/Kotlin APK; public or
  general JNI, APK/AAR distribution, lifecycle/UI, device, general-corpus,
  public-admission, and `SPX-B104` claims remain closed.
- Added the separate private [Android JNI ownership adapter
  v1](docs/ANDROID-JNI-OWNERSHIP-V1.md) implementation and a dedicated hosted
  APK job. A feature-gated generator emits exact x86_64/arm64 provider and JNI
  shim sources; strict NDK compilation, exact `JNI_OnLoad` export/dependency
  inspection, and a plugin-free Gradle 9 `--offline` packaging project produce
  one same-package, no-UI framework Instrumentation APK containing only the
  x86_64 JNI library and O0/O2 providers. The minSdk-28 Kotlin wrapper uses an
  owning `HandlerThread`, generation-tagged `SPXAJH01` handles, fixed
  `SPXAJS01` status words, and a `PhantomReference`/`ReferenceQueue` fallback.
  `OwnedSession.consume()` is the explicit evidence path;
  `AutoCloseable.close()` is non-throwing Cleaner-style dispatch. Tests invoke
  the identical registered cleanup action deterministically through
  `cleanForTest()`, not by observing GC. Local Rust/C and source-lock evidence
  plus the exact API-35 x86_64 APK/Instrumentation path are green in [run
  31324497016, job 93272580149](https://github.com/wavect/semaprax/actions/runs/31324497016/job/93272580149).
  This is no AAR, UI/lifecycle, device, arm64 runtime, general resource
  execution, public admission, or `SPX-B104` change.
- Added a mandatory private arm64 iOS Simulator runtime gate for one exact
  `token.discard-two` callable-v3 provider. The closed emitter produces the
  target-specific descriptor and strict-C provider; CI links it with the
  static-only loader/host, ad-hoc signs the standalone Mach-O, and requires
  exact O0/O2 finalizers, authenticated no-owned receipt/ledger transition, and
  zero measured Rust allocation across the irreversible interval. The first
  hosted Simulator job is green in [run 31318280135, job
  93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836);
  device/app
  lifecycle, the remaining iOS corpus, Android, and public admission remain
  open.
- Added iOS-target cfg isolation plus a mandatory macOS CI cross-check for the
  unpublished static callable-v3 loader and receipt/ledger host across five
  distinct device, simulator, and Catalyst Rust targets. The gate requires zero
  resolved `libloading`, dynamic `open_*`, or desktop v1/v2 host surface there;
  linking, mobile runtime, Swift/XCFramework integration, and public admission
  remain open.
- Added resource declarations and explicit `own`, `borrow`, and `shared` boundaries.
- Added stable resource identities and atomic resource/type-boundary renames.
- Added straight-line move analysis with use-after-move and illegal-transfer diagnostics.
- Added lexical `let` bindings, typed `if/else`, and conservative control-flow ownership joins.
- Distinguished definitely moved resources from resources moved on only some paths.
- Exposed resource nodes and parameter ownership in the semantic graph.
- Upgraded the graph to v2 with deterministic revision-scoped expression and binding nodes.
- Added structured contract graphs and contract dependencies to bounded agent context.
- Added a direct WebAssembly core backend and generated browser package.
- Preserved checked `i64` arithmetic through audited WebAssembly host imports.
- Added the authoritative full-goal completion matrix.
- Verified the compiler, native executable, and WebAssembly package on macOS, Linux, and Windows CI runners.
- Made native expression evaluation explicitly left-to-right instead of inheriting C's unspecified call-argument order.
- Hardened public backends to reject unverified programs with diagnostics instead of panicking.
- Added repository agent guidance, evidence rules, MSRV/package gates, and a documented Graphify adoption decision.
- Added a fail-closed resolved HIR with persistent nominal/call identities, deterministic lexical place identities, and centralized type facts/layout keys.
- Migrated native and Wasm semantic lowering to validated HIR and added malformed-HIR cross-backend rejection parity.
- Upgraded the semantic graph to v3 backed by validated HIR, with resolved declaration/value/type identities, centralized type facts, explicit identity origin, fail-closed public APIs, and bounded-context frontier metadata.
- Upgraded the semantic graph to v4 with persistent record and field nodes, resolved construction/projection references, recursive field-type context closure, and fail-closed graph integrity checks.
- Replaced FNV revision tokens atomically across graphs, semantic patches, CLI output, and `semaprax.web.v2` manifests with domain-separated SHA-256 content addresses.
- Split source verification behind a compatibility facade and additive HIR analysis API while freezing complete ordered diagnostic JSON behavior.
- Added canonical record declarations, construction, projection, persistent field identities, deterministic field diagnostics, recursive facts/layout keys, and by-value recursion rejection.
- Upgraded the semantic graph to v4 with record/field nodes and stable constructor/projection references; native and Wasm record builds fail closed pending aggregate cleanup and layout support.
- Added prefix-aware ownership for resource-containing record fields, preserving disjoint siblings while rejecting definite and conditional partial-place reuse in both source verification and hostile-HIR replay.
- Fixed canonical formatting of record constructors in contracts and `if` conditions so parse-format-parse remains valid.
- Hardened semantic resource renames so record initializer expressions cannot be mistaken for type annotations.
- Published design-only RFC 0003 for exactly-once cleanup, logical resource imports, and a shared native/Wasm status-and-out ABI; executable cleanup remains gated on its conformance evidence.
- Closed the pre-cleanup backend gap by rejecting bare resource modules with `SPX-B104`/`SPX-W111`; record diagnostics retain precedence when both declaration kinds are present.
- Implemented RFC 0003 phase 1 with mandatory persistent trivial/imported resource lifecycles, declaration-only interface/import contracts, recursive lifecycle-effect authority checks, and hostile-HIR validation while retaining fail-closed resource execution.
- Upgraded the semantic graph to v5 with resource-drop, interface, and logical-import nodes plus lifecycle-aware bounded context closure and exact snapshots.
- Migrated legacy resource fixtures explicitly and extended atomic resource renames through import parameter types without rewriting lifecycle IDs or logical keys.
- Added a mandatory, deterministic `CleanupInventory` to resolved functions, cataloging owned droppable storage and exact nested resource-leaf flags while independently rejecting hostile inventory mutations before backend gates.
- Corrected the proposed cleanup-plan schema to require atomic call commits, exact single-flag finalizer guards, explicit entry liveness, edge-based cleanup continuation, and sticky failure-status identities; executable cleanup remains unimplemented.
- Implemented RFC 0003 phase 2 with a mandatory target-neutral cleanup CFG on every resolved function, covering all current HIR expressions, lexical exits, guarded reverse finalization, caller-owned argument epochs, atomic call commits, checked and contract failures, partial record construction, whole-value normalization, and scalar/owned result publication.
- Added independent cleanup-plan reconstruction after core HIR and inventory validation, plus focused deterministic and hostile-HIR tests that preserve `SPX-H006` precedence across native and Wasm consumers.
- Upgraded the semantic graph to v6 with complete tagged cleanup plans per selected function while retaining the canonical source revision algorithm.
- Added public `semaprax.status.v1` normalized-status types, exact compiler-owned contract/arithmetic mappings, and a bounded context-local immutable status arena; token zero remains success and no physical token is serialized. This is protocol/runtime groundwork, not a backend status ABI implementation.
- Added public `semaprax.conformance-trace.v1` semantic event/result types and deterministic canonical JSON for ownership transitions, calls/imports, frame-local failure selection, infallible finalization, and result publication.
- Added independent attached-plan coverage and exhaustive current-CFG replay plus a scenario-driven single-frame reference executor with guarded cleanup, sticky normalized failure, exact trace emission, and explicit caller out-slot publication state. Recursive calls, callable imports, and native/Wasm instrumentation remain unimplemented, so no backend conformance is claimed.
- Documented strict status/trace schema rejection, compatibility, cache-binding, event-order, and no-physical-data rules in [Conformance trace v1](docs/CONFORMANCE-TRACE-V1.md).
- Migrated native scalar calls to the RFC 0003 context/status/out convention: internal contract and checked-arithmetic failures now propagate exact normalized statuses without terminating a SEMAPRAX frame, nested calls retain the same token, and caller result storage is written only after successful postconditions.
- Added an executable strict-Clang native ABI matrix covering scalar success, requires/ensures, all eight arithmetic status codes, left-to-right nested failure propagation, arena shape, and poisoned out-slot preservation while retaining the `SPX-B104` resource gate.
- Fixed status-v1 domain identity at 1–255 UTF-8 bytes without NUL and enforced the same byte rule in public status construction, source/HIR validation, and native arena-owned domain storage.
- Added gated native-resource ABI scaffolding with deterministic stable-ID-derived C wrapper and typed finalizer symbols; this does not enable resource execution.
- Added a fail-closed first-slice cleanup-plan index for direct trivial resources and a checked max-path trace-capacity preflight. Records, imported lifecycles, projections, generics, and every nested call remain rejected behind `SPX-B104` until their executable conformance evidence exists.
- Added an unreachable plan-driven native cleanup C scaffold with exact terminal liveness/status assertions, clear-before-trivial-finalization, owned-result publication checks, and a compiler-owned C binding namespace; executable resource lowering remains gated.
- Added deterministic, strongly typed C wrappers for direct opaque resources and staged resource-aware native signatures while preserving the unconditional `SPX-B104` execution gate.
- Extended the gated native cleanup scaffold to emit real root-frame `transfer`, `select_failure`, trivial-finalization, and `result_commit` events from the classified function identity; strict C11 fixtures cover event order, hostile trigraph sequences, and exact UTF-8 identity bytes.
- Made every persistent semantic identity NUL-free at source and hostile-HIR boundaries, including type/expression/place references and attached cleanup inventory/plan metadata, so C and wire encodings cannot silently truncate or alias identities.
- Added an exact test-only native cleanup conformance lane: a bounded versioned binary decoder and validated-HIR identity materializer compare typed traces and canonical JSON with the independent executor across zero/max opaque payloads, reverse finalization, contract/arithmetic failure, owned publication, failed owned postconditions, O0/O2, ASan, and UBSan. Production resource lowering remains gated.
- Replaced injected native cleanup observations with a typed, cleanup-CFG-synchronized value planner that executes real Boolean contracts, `i64` comparison, checked addition, scalar publication, and owned transfers inside the exact conformance lane. Added portable `i64::MIN` C emission and independent resource-lifecycle/transfer-type coherence checks; the public resource host gate remains closed.
- Added the private host-ownership transaction v1 reference model: linked-runtime-unique non-clone registries, generation/provenance tokens, immutable function contracts, atomic multi-owner ingress, and must-complete typed execution scopes make rejection versus executed success/failure and owned-result publication executable without exposing raw pointers or weakening `SPX-B104`.
- Split private native host contracts into deterministic authority-free templates and runtime binding. Resource preflight now derives and discards each template from its exact already-admitted cleanup/value evidence without replanning; complete ordered scalar/resource metadata, exact same-type owner results, lifecycle identity, module ABI and function-template fingerprints, mismatched-evidence rejection, binding-instance-distinct process-local authority, cross-ABI binding rejection, and internally observed thread affinity at binding and synchronous registry execution are unit-tested while public execution remains gated.
- Corrected the native conformance probe to consume the value planner's exact owned-result parameter/owner ordinal instead of choosing the first same-typed input. The sanitizer-backed corpus now proves returning the second of two identical resource types with both distinct and identical opaque payloads, plus reverse cleanup and no result publication when its precondition fails.
- Added a private descriptor-only physical native ABI stage derived solely from sealed admitted host templates. Its canonical pointer-free wire binds explicit schema, target, semantic/physical module, function, ordered scalar/resource/lifecycle, and exact result identities. A host-only provider compile-guards every encoded target property; strict separate C11/C++ translation units plus a real shared-library/export/dynamic-consumer test verify the deterministic getter as the sole export. Compiler preflight discards every staged artifact, exports no callable owner API, creates no runtime authority, and leaves the exact public `SPX-B104` gate unchanged.
- Initially added the private native capability-token codec as a disconnected exact 64-byte canonical envelope with a full RustCrypto HMAC-SHA256 tag. Pinned audited crypto, a published RFC 4231 vector, independently reproduced owner/result full-token goldens, every-bit/arbitrary-byte/length/structure hostility, exact function-template result scoping, function-independent owner scoping, cross-context rejection, stale/max-generation checks, and explicit entropy/module-lifetime/linearity nonclaims established the mechanics later connected by the callable host.
- Added a private OS-backed native capability authority without connecting it to compiler preflight or any export. Exactly pinned `getrandom` 0.4.3 supplies one fail-closed seed for the secret, nonzero random epoch, and opaque thread-binding nonce; kind-specific non-formatting credentials seal immutable module/resource context and reject every operation off the actually captured Rust thread. Test-only deterministic entropy, independently reproduced authority goldens, error/zero/context/thread hostility, MSRV, and desktop OS smoke preserve the public `SPX-B104` gate while module retention, ledger integration, fork safety, and callable ownership remain blocked.
- Added a private fake-backed `NativeModuleLease` topology and required the native capability authority plus every staged owner/result credential wrapper to retain the exact allocation instance. Tests cover equal-fingerprint instance separation, process-incarnation rejection, one-way draining, lease-derived fingerprints, cross-instance rejection despite equal bearer bytes, drop-order retention, concurrent final release, and absence of retention cycles. There remains no production constructor, platform loader handle, code-identity admission, physical pin/unload protocol, ledger integration, callable export, or change to `SPX-B104`.
- Added an unpublished `semaprax-native-loader` workspace quarantine around the unavoidable trusted-library load, fixed-getter lookup, and bounded descriptor-read/compare unsafe edge. At this initial stage its same-thread opaque explicit-retain lease, exact-pinned `libloading` 0.8.9, compile-fail trait checks, workspace-wide gates, and real Linux/macOS fixtures were isolated from the compiler and authority; later entries connect them privately while the public adapter remains gated.
- Added a blocking, immutable-action-pinned `cargo-deny` CI gate for the complete native/mobile/Wasm dependency graph: RustSec advisories, unapproved licenses, duplicate or wildcard versions, Git dependencies, and registries outside crates.io fail the build with no advisory exceptions.
- Added a root-frame native trace-storage scaffold with exact compiler-status/event validation and a pre-ownership attachment handshake. Canonically zeroed one-shot contexts, buffers, and event slots use owner/generation checks to reject rebinding, aliasing, double attachment, and capacity underflow before execution.
- Added the unpublished `semaprax-native-host` physical ownership stage. It strictly decodes compiler-derived descriptors, retains the exact real loader instance through its same-thread OS-seeded authority and opaque owners, authenticates owner/result credentials, connects the private ownership ledger, preserves owners on precommit rejection, rotates owned results, and gates new work after draining. The later callable-v2 work below replaces its original trusted-closure execution fixture; compiler resource builds still retain `SPX-B104`.
- Added the first narrow public `semaprax.wasm-owned.v1` Core Wasm execution path for one direct trivial-resource identity. Generated adapters consume replay-validated terminal cleanup order, stage owner handles atomically, normalize contract/arithmetic/adapter status records, preserve poisoned result storage on failure, rotate owned results, and reject excluded shapes with `SPX-W111`. The generated JavaScript keeps host imports private, binds calls to exact generated metadata and SHA-256-authenticated Wasm bytes, rejects non-canonical ABI arguments, checks result ranges before ownership commit, and uses one-shot trusted adoption tickets; Node tests cover the admitted slice. This is not WebAssembly Component resources or production native-host conformance.
- Added `semaprax.semantic-event-dictionary.v1`, which assigns deterministic
  nonzero ordinals to exact semantic event shapes. Generated cleanup C and the
  real Wasm owned adapter emit those ordinals from their executed control flow;
  the host-side materializer rejects zero or unknown ordinals without inferring
  or repairing events.
- Unified the authoritative direct-trivial-resource conformance corpus at 14
  named scenarios. Real compiler-generated native shared libraries at O0/O2 now
  execute through the exact loader/authority/ledger ownership host, while real
  Node/Wasm executes the same cases. Both materialize to the exact reference
  trace and normalized outcome for zero/max payloads, reverse cleanup, contract
  and checked failures, scalar publication, owned identity/selection, and failed
  owned publication; native also proves result rotation and final logical
  liveness.
- Added private callable native descriptor v2, derived from the sealed compiler
  host template plus execution/cleanup, semantic-dictionary, and trace-path
  evidence. Its canonical pointer-free wire binds twelve fingerprints, exact symbols,
  request/response capacities, complete ordered signature, opaque-`u64` owned
  payload kind, and result mapping. The unpublished host's independent strict
  parser accepts compiler output and rejects every single-byte mutation,
  truncation, and trailing byte.
- Extended the native loader quarantine with Unix `RTLD_NOW | RTLD_LOCAL`, exact
  callable-v2 symbol admission, bounded preallocated one-shot byte calls, and
  exact-instance rejection, then connected that transport to the ownership host
  with strict request/response codecs and allocation-free postcommit decoding.
- Added `semaprax.trace-path-certificate.v1`: the compiler deterministically
  compiles every admitted cleanup path into a canonical trie-DFA separately
  fingerprinted into descriptor v2, symbols, and call contracts. Host admission
  authenticates it and rejects omitted, duplicated, reordered, or wrong-outcome
  traces before semantic materialization.
- Added complete private callable-provider emission with exact physical result
  and outcome namespaces, owned-payload integrity checks, and compile-time
  architecture/OS/environment/object/pointer/endian guards. Exact MSVC/GNU
  source known answers and deliberate target/payload mismatch fixtures fail
  closed without touching the response.
- Made formatting, Clippy, tests, docs, builds, and the Rust 1.85 gate run every
  workspace feature so staged production surfaces cannot escape CI. `SPX-B104`
  remains closed for general physical/malformed-response fallback cleanup and
  quiescence, Android/iOS profiles, and public native execution/admission.
- Added a separate fail-closed Linux Rust-host ASan lane pinned to
  `nightly-2026-07-16`: it rebuilds the target standard library, proves active
  Rust instrumentation with both an intentional fault and binary/compiler
  inspection, and runs the real callable host plus generated corpus. The exact
  lane passed in [public run 31259216533, job
  93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065),
  alongside a fully green current hosted-CI matrix for Linux, macOS, Windows,
  MSRV, dependency policy, and provider sanitizers. This is not mobile or app-
  platform evidence. Rust-host UBSan is not claimed and `SPX-B104` remains
  closed.
- Recorded the green public Linux
  [callable-host sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801):
  all 14 authoritative O0/O2 cases executed from dynamically loaded
  ASan/UBSan-instrumented generated providers through the Rust host. The Rust
  host code itself was not sanitizer-instrumented. The dependency-policy job was
  also green, but unrelated Clippy/GCC failures stopped the platform jobs before
  runtime evidence and kept the overall workflow run red; no Windows runtime
  evidence is inferred from this job.
- Added the hidden callable-v3 settlement foundation and proposed RFC 0004: a
  bounded target-neutral certificate/frame/receipt model with one all-live
  start, typed progress, exhaustive owner-state enumeration, exact accept/abort
  cleanup actions, idempotent quiescent receipts, deterministic fingerprints,
  and hostile mutations. A private compiler deriver now preserves exact HIR
  result-staging/finalization timing and binds terminal settlement to accepted
  semantic trace paths for the authoritative 14-case direct-trivial corpus.
  The model deliberately provides no invocation or module-instance reservation,
  physical finalizer authority, descriptor/provider, loader/host, public
  compiler, or backend runtime evidence; `SPX-B104` remains closed.
- Added a private linear settlement transaction model with closed `Executing`,
  `DecisionLocked`, `ActionInProgress`, `ProviderSettled`, model
  `ReceiptCommitted`, and absorbing `Quarantined` phases. Its 29 focused tests
  cover phase-aware unwind, every-finalizer interruption without retry, exact
  candidate/committed replay, hostile mutation/cross-binding, and preserved
  evidence while keeping provider `Published` unauthoritative. The model
  allocates and grants no exact-instance reservation, host authentication,
  ledger publication, FFI/provider, or physical-finalizer authority; this
  changes no v2 bytes or public/runtime gate.
- Added private callable settlement-proof v1 without consuming the future v3
  ABI version. `SPXNPRF1` embeds the exact unchanged callable-v2 descriptor and
  a canonical pointer-free binary settlement graph under one 64 KiB ceiling.
  Separate compiler and host codecs bind the exact v2 call contract and trace
  certificate, reject rehashed cross-module/changed-trace substitutions and
  hostile graph structure, and reproduce a fixed known answer. The compiler
  enforces the cap while serializing; the v2 loader rejects proof magic before
  opening an image; default consumers cannot import the proof surface. This adds
  no provider, descriptor-v3, loader admission, host settlement execution,
  physical finalizer, public API, mobile evidence, or `SPX-B104` change.
- Froze the private [callable ABI v3 descriptor/wire
  contract](docs/NATIVE-CALLABLE-ABI-V3.md): `SPXNABI3` fixes the descriptor,
  acyclic hash dependencies, recovery graph, capacity budget, dynamic and
  iOS-static linkage roles, six complete provider codecs, and the distinct
  host-only 524-byte committed-receipt codec. The six-argument execute ABI,
  payload-bearing frame cells, closed tags, request/response/decision/action/
  frame/candidate digests, and separate receipt-key HMAC replace the former
  provisional identities and freeze changed private v3 known answers.
  `CertifyOutcome` embeds the canonical ordinal/outcome witness and a nonzero
  trace-certificate-bound evidence digest independently recomputed by the host;
  this binds the witness only and is not host acceptance of the trace-path DFA
  certificate. Resealed witness or digest mutations are rejected.
  Independent compiler encoders and host parsers cover those seven complete
  transcripts. The emitter is bound to its
  build target and provides no Android/iOS/Windows cross-emission evidence. Both
  existing loader constructors now reject v3 magic before path
  canonicalization or image/symbol access, including malformed same-magic
  headers. The compiler/host codec tranche grants no provider,
  loading, settlement, finalizer, ledger, mobile, or public authority and leaves
  v2/proof bytes and `SPX-B104` unchanged.
- Added the first private callable-v3 physical components: two bounded generated
  strict-C11 providers execute scalar-discard and owned-identity settlement at
  `-O0`/`-O2`; an exact dynamic-image loader verifies root provenance for the
  getter, execute, settle, and descriptor storage; and the host has a distinct
  OS-seeded receipt authority with a fixed-capacity atomic ledger/facade. These
  components are not yet connected by one host invocation and do not prove the
  full 14-case physical corpus, exhaustive failure injection, sanitizers,
  Windows v3 runtime, Android/iOS, quiescence, malicious-code containment,
  public admission, or any `SPX-B104` change.
- Connected one private callable-v3 path from compiler-generated strict C
  through exact dynamic-image admission into the authoritative host receipt
  ledger. Scalar discard-two and owned identity execute at `-O0`/`-O2` with
  exact buffer handoff, independent host decoding, finalizer order, receipt
  replay, generation refresh, cross-instance rejection, and unload pinning.
  Separately, graph-derived providers now execute all 14 authoritative normal
  corpus scenarios at `-O0`/`-O2`, including mixed owned/scalar/bool inputs,
  exact trace/action digests, dispositions, and replay; a mandatory ASan+UBSan
  gate and explicit Windows gate are configured. Pending/pre-execute host
  unwind fails closed without effects because its canonical returned-response
  transcript is not yet frozen. The joint host still allocates during
  post-`CallCommit` evidence decoding/replay, and the full corpus has not yet
  run through loader plus host. Failure injection, public CI observation of
  this batch, mobile/static admission, quiescence, malicious-code containment,
  and public `SPX-B104` admission remain open.
- Expanded the private callable-v3 joint path to all 14 authoritative scenarios
  at `-O0`/`-O2`, with mixed scalar/bool/owned requests, exact graph-derived
  finalizer evidence, authenticated replay/publication, and unload pinning. A
  counting allocator observes zero Rust heap growth from immediately before
  `CallCommit` through `ReceiptCommit`; injected reusable-decode reserve failure
  quarantines exact bytes, owners, and the image pin. Seven provider-side
  failure fixtures cover physical return, malformed response/frame/candidate,
  durable finalizer interruption, replay, and decision conflict at `-O0`/`-O2`
  and under ASan+UBSan. Pre-execute unwind, fatal allocator/process-crash
  recovery, hosted Windows confirmation, mobile/static admission, quiescence,
  malicious-code containment, public admission, and `SPX-B104` remain open.
- Added canonical pre-execute callable-v3 unwind: frame tag 3 and reserved
  `0xFFFF_FFFE` bind exact zero-filled response storage, the loader never enters
  provider execute, certified abort finalizers settle, and the host commits an
  authenticated receipt without Rust heap growth. Expanded all seven physical-
  failure/interruption fixtures through exact loader and host at `-O0`/`-O2`,
  including quarantine boundaries and zero retry. Added a bounded process-
  lifetime iOS-static exact-address registry that shares the dynamic host's
  generation/draining/quarantine ledger without any path, `dlopen`, close, or
  unload surface. Non-Apple static-function evidence is green; representative
  iOS/Android execution, fatal allocator/process-crash recovery, quiescence,
  public admission, and `SPX-B104` remain open. Private v3 descriptor/frame KATs
  migrated; v1/v2/proof bytes remain unchanged.
- Added a mandatory Windows callable-v2 dependency-isolation fixture. It places
  a same-name dependency in both CWD and legacy `PATH`, proves the root-image
  sibling wins for descriptor admission and invocation, then removes that
  sibling and requires `LibraryOpen` rather than malicious fallback. CI names
  this fixture and the complete O0/O2 callable corpus explicitly. Both passed
  in [run 31257545008, job 93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756).
- Added a public build-only native-callable API and CLI for one explicitly
  selected direct-trivial owned function. It produces a deterministic hashed
  provider bundle and strict host shared library through safe staging and
  observed no-overwrite checks, while exposing no loading, invocation,
  adoption, or authority and retaining ordinary native `SPX-B104`.
- Added a private bounded iOS-static callable-v3 registration alternative. It
  binds one exact descriptor-storage/getter/execute/settle address tuple to a
  process-lifetime logical instance, makes exact bootstrap re-registration
  idempotent, rejects target relabeling and partial address reuse, and feeds the
  same generation/draining/quarantine ledger as dynamic admission. Non-Apple
  fake-function evidence exercises physical-return quarantine and pin
  retention; there is no `dlopen`, close, unload, device-runtime, public API, or
  `SPX-B104` claim.
- Migrated browser manifests from `semaprax.web.v2` to `semaprax.web.v3`. Version 3 retains module, graph revision, Wasm entry, and capabilities while adding the required `owned_abi` object with schema `semaprax.wasm-owned.v1` and a declaration-ordered function mapping; scalar-only packages use an empty function array. Version-2-only consumers must reject or explicitly migrate rather than inferring ownership ABI metadata.

## 0.1.0 — 2026-08-07

- Introduced the typed SEMAPRAX source subset and canonical formatter.
- Added stable semantic identities, revisioned graph output, and context slicing.
- Added effect/capability verification and typed runtime contracts.
- Added checked arithmetic and native C11/Clang compilation.
- Added machine-readable diagnostics and atomic semantic rename patches.
- Published RFC 0001 and the staged compiler roadmap.
