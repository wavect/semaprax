# Compiler architecture

SEMAPRAX v0.2 is a vertical slice through the future compiler. Each layer has a narrow contract so the prototype can grow without turning its source syntax into its internal API.

```text
.spx source
    |
lexer -> parser -> parsed AST
                    |
              semantic verifier
                    |
                resolved HIR
                     |
         replay-validated cleanup plan
                /    |     \
 semantic graph  C11 IR  Wasm core
    /       \             |           |
 agent queries    tx    Clang     browser host
                         |           |
                  native executable  web package
```

## Source projection

`lexer` and `parser` accept a deliberately small grammar. `format::canonical` is the single source projection. Graph revisions hash this canonical form rather than incidental whitespace, so formatting-only edits do not invalidate an agent transaction. The cross-protocol revision token is `sha256:<64 lowercase hex digits>` over `b"semaprax.graph-revision.v1\0" || canonical_source_utf8`. It is collision-resistant content addressing and stale-base detection, not a signature or MAC.

Declarations should carry an explicit `@id`. Automatic identities are accepted for exploration but produce `SPX-S103`, because a name-derived ID cannot survive a rename.

## Verification

The public verifier compatibility facade and HIR analysis boundary share one source verifier, preserving the established ordered diagnostics while removing a `hir -> verify` dependency cycle. Warnings-only analysis retains diagnostics and still produces resolved HIR; any source error fails closed before lowering. The verifier builds the module symbol table and checks:

1. Unique names and stable IDs.
2. Parameter, expression, call, and return types.
3. Boolean preconditions and postconditions.
4. Function effects against module permits.
5. Transitive effect declarations at each call edge.
6. Lexical local scope and conservative ownership joins across branches and lazy boolean paths.
7. Explicit resource lifecycle cardinality, interface/import authority and failure contracts, and lifecycle effects on every function that can own a resource.
8. The native entry-point shape.

The initial contract lane is progressive: contracts are type-checked at compile time, required to be effect-free, and guarded in generated native and Wasm code. Static proof is a later lane, and its absence is reported honestly in the project status.

Generated arithmetic is checked for overflow, zero division, and the signed division edge case. Failures have stable process exit codes and explicit diagnostics rather than C undefined behavior.

## Resolved HIR groundwork

`hir` is the fail-closed boundary between verified human syntax and future semantic consumers. It resolves nominal types, resource lifecycles, interfaces, logical imports, record fields, projections, and calls through persistent declaration IDs, assigns deterministic structural identities to parameters, locals, expressions, and result values, and represents rooted field references as places. Spans and display names remain diagnostic metadata rather than semantic identity.

The declaration index is the single current source of target-independent type facts: whether a type is copyable, contains resources, is sized, needs destruction, and its name-independent layout key. Generic parameter identities include their owner declaration plus index, and nominal identities include the complete resolved argument tree.

The native and Wasm emitters now consume only validated HIR for semantic lowering; their parsed-AST entry points are compatibility wrappers that resolve first. A centralized HIR validator rejects duplicate/non-canonical identities, invalid declarations and nominal types, lexical-scope violations, inconsistent expression/call types, definite or conditional resource reuse, contract transfers, undeclared or unpermitted effects, effectful contracts, invalid result bindings, and an invalid entrypoint before either backend emits an artifact. Only after those checks pass, `cleanup` independently rebuilds the structural storage inventory; `cleanup_plan` then rebuilds and exact-compares the complete target-neutral plan. A hostile direct-HIR transform therefore cannot remove, retarget, reorder, or forge cleanup meaning before reaching Graph or a backend.

This remains staged groundwork rather than the sole compiler IR: the current verifier still establishes meaning from parsed AST before HIR resolution. Explicit trivial/imported resource lifecycles, declaration-only interface/import contracts, record declarations, constructors, projections, stable identities, recursive resource/type facts, and by-value recursion rejection now reach validated HIR and Graph v6. The source checker and HIR validator independently replay lifecycle compatibility, lifecycle-effect authority, and prefix-aware partial-place availability. `CleanupInventory` remains a structural discovery boundary. Every `ResolvedFunction` additionally carries `semaprax.cleanup-plan.v1`: typed blocks, edges, lexical regions, entry liveness, storage/leaf flags, atomic call commits, sticky status sources, guarded finalizers, and scalar/owned result publication. The builder covers every current HIR expression and normal/checked-failure path; the validator reconstructs the plan from core HIR rather than trusting attached metadata.

The target-neutral runtime protocol is split from physical target state. `semaprax.status.v1` contains only a stable domain, nonzero code, class, and retryability; the invocation-local arena assigns immutable one-based tokens while reserving zero for success and rejects cross-context and same-nonce cross-arena resolution. `semaprax.conformance-trace.v1` records semantic ownership, import, write-once failure-selection, finalization, and result-publication events without pointers, handles, tokens, offsets, or host exceptions. Automatic-finalizer completion is success-only in the Rust type model. Attached plans are independently checked against inventory and exact typed-HIR control/event coverage, then exhaustively replayed across the current acyclic CFG for ordered liveness, sticky failures, exact region-leave chains, reverse cleanup, and typed whole-result publication. Iterative reachability plus deterministic 65,536-path and program-wide 1,000,000-work-unit limits turn hostile amplification into `SPX-H006` rather than recursion failure or unbounded memory growth. A deterministic scenario executor emits the expected single-frame scalar/resource trace with an explicit uninitialized/published caller out-slot model; record results are rejected until the trace schema can preserve aggregate semantic values. The native scalar C lane shares one caller-supplied context across nested calls, returns exact compiler statuses, and commits its out-slot only after postconditions. A private host-ownership reference ledger proves atomic preflight/commit and must-complete scalar/owned completion semantics without raw pointers. For the admitted native slice, resource preflight now derives and discards a deterministic, authority-free host template from the exact already-admitted cleanup/value proof: complete signature order, lifecycle and result identity, module ABI fingerprint, and function-template fingerprint are fixed without replanning. That sealed template is the only input to the private [native adapter descriptor](NATIVE-ADAPTER-DESCRIPTOR-V1.md): a canonical pointer-free byte string bound to its schema, physical target, semantic module ABI, function template, ordered parameters, lifecycles, and exact result mapping. The staged host-only provider compile-guards the encoded architecture, OS, environment, object format, pointer width, endianness, and getter ABI; strict tests build and load a real shared library and allowlist its sole getter export. A separate private [capability-token layer](NATIVE-CAPABILITY-TOKENS-V1.md) defines authenticated bearer bytes for a future retained runtime: owner tokens remain function-independent, while provisional owned results bind the exact function-template fingerprint. Its ledger-disconnected authority obtains one 72-byte seed from the native OS source, rejects errors and all-zero structural components without fallback, captures the actual Rust thread, and seals its secret, random epoch, module/resource context, and opaque thread-binding nonce behind kind-specific methods. Under tests, construction also consumes a fake-backed module lease, derives the physical fingerprint from it, and makes every staged credential wrapper retain the exact same `Arc` allocation. Equal fingerprints and equal bearer bytes do not establish instance identity. Test-only deterministic injection and exact independent goldens cover the construction boundary. This supplies neither token linearity, fork recovery, nor physical module retention: there is no production lease constructor or OS loader handle, and compiler preflight never constructs it. Detached evidence and cross-ABI template bindings fail closed. Physical public adapters, code-identity admission, quiesced unload, imported finalizers, synchronized ledger integration, and the equivalent Wasm ABI remain required before a backend-conformance claim.

## Record groundwork and backend gate

Canonical source accepts nominal records with persistent field IDs, source-ordered construction, shorthand expansion, and chained projection. The verifier reports unknown, duplicate, missing, or mismatched constructor fields deterministically and rejects direct or indirect by-value layout cycles. Resolved HIR distinguishes place projections from projections of temporary values, and its validator rejects foreign/reordered fields and inconsistent facts.

Resources now require one explicitly identified `drop trivial` or `drop import` strategy. Imported strategies resolve through an explicitly identified interface/import contract with ownership, authority, consumption, result-publication, and failure meaning; the v1 source grammar uses the import `@id` as its logical key while HIR keeps those concepts separate. Resources and records containing resources remain semantically non-copy. The compiler has a shared cleanup plan plus target-neutral reference replay/execution, and the native scalar lane has a non-trapping status/out ABI. A gated native test lane executes the admitted direct-trivial-resource value/cleanup slice and compares exact traces with the independent oracle; the private host ledger defines safe ownership transaction semantics and the private physical descriptor makes admitted compatibility evidence deterministic. Neither native nor Wasm publicly hosts resources yet: the descriptor contains no callable owner API, imported calls are not expressions, physical finalizer adapters are absent, and no production backend claims resource conformance. Native and Wasm therefore continue to reject every resource-bearing or record-bearing module; record diagnostics retain precedence. RFC 0003 phases 1–2 are implemented; current native evidence is phase-3 groundwork, not public backend resource conformance.

## Semantic graph

`semaprax.graph.v6` is serialized exclusively from validated resolved HIR. It contains the canonical human-source revision, module capabilities, entrypoint declaration ID, explicit-versus-automatic declaration identity origin, resource/lifecycle/interface/import/record/field/function nodes, typed ownership and result-publication contracts, effects/authority/failure meaning, structural contracts, sorted declaration-ID call dependencies, the structural body graph, and the complete cleanup plan for every selected function. Cleanup vectors preserve canonical execution order and use tagged storage/place/status/transition/edge/region/exit forms; serialization never sorts malformed input into apparent validity. Context closure follows a resource through its lifecycle, full interface contract, import signatures, nominal types, and each selected function's self-contained cleanup plan. The source revision algorithm is unchanged, so caches key by both graph schema and revision.

Integer literals are decimal JSON strings rather than JSON numbers, so JavaScript and TypeScript agents preserve every `i64` value exactly. `let` bindings expose one value identity; the enclosing statement does not reuse that ID as a second identity domain.

Expression and value IDs are deterministic but revision-scoped; only explicitly authored declaration IDs are persistent across revisions. Automatic name-derived declaration IDs remain visibly marked unstable. Spans are intentionally absent because canonical source revisions ignore whitespace while spans do not.

Context slicing starts from a display name or exact declaration ID, with exact IDs taking precedence on collisions. It walks declaration-ID call dependencies from preconditions, bodies, and postconditions to a bounded depth and closes referenced record types transitively through their field types. Unrelated declarations remain excluded. Every result declares a `module` or `context` view. Context views record root, depth, truncation, and frontier IDs so an omitted call dependency is distinguishable from a dangling reference. Callers, tests, packages, targets, and generated artifacts will become additional typed edges.

The public parsed-AST graph functions resolve and validate HIR and return diagnostics on failure. Direct HIR rendering remains internal so a caller cannot attach a forged canonical-source revision to transformed HIR. The graph currently rebuilds per command; a later daemon will persist indexed revisions.

## Transactions

The `.spatch` protocol is intentionally smaller than a text patch:

```text
base <graph-revision>
rename <stable-id> to <new-name>
require no-new-effects
```

Application is all-or-nothing:

1. Parse the current source and compare its canonical revision.
2. Resolve operations through stable IDs.
3. Apply declaration and call-edge changes in memory.
4. Reparse and verify the candidate program.
5. Evaluate patch requirements.
6. Canonically render to a sibling temporary file.
7. Atomically rename it over the original.

The protocol will evolve toward structured JSON/CBOR operations with typed payloads, affected-node proofs, target requirements, and multi-file commits.

## Native backend

The v0.2 native backend emits readable C11, then invokes Clang. C is an implementation IR, not a promised ecosystem boundary. This gives the prototype real native binaries, easy inspection, sanitizers, and broad host support with almost no backend dependency surface.

The C emitter materializes subexpressions in source order before calls and operators. This is required because C does not define function-argument evaluation order, while SEMAPRAX does. Lazy boolean operators and `if` expressions lower to explicit branches, and generated local names cannot collide with source identifiers. Every scalar function uses `(context, parameters..., result_out) -> status_token`: nested calls share one invocation-local arena, zero means success, contract and checked-arithmetic failures return exact normalized records, and `result_out` is written only at the final success commit. The executable wrapper translates a completed root failure back into the existing process diagnostics and exit codes.

The planned development backend is Cranelift. The planned optimizing pipeline uses multi-level IR with LLVM, while portable components lower through the WebAssembly Component Model. Backend changes must preserve the graph and verification contracts.

## Ownership seed

Resource declarations and records containing resources introduce non-copy semantic values. Function parameters state whether they receive ownership, borrow for the duration of a call, or participate in explicit shared ownership. `let` and record construction transfer owned values left-to-right while preserving borrowed/shared modes. Moving an owned record field invalidates that place and its parents while leaving disjoint siblings available. The verifier joins field state across `if` and lazy boolean control flow, distinguishes definite from conditional partial moves, replays the same rules at the public HIR boundary, and prevents borrowed/shared fields from crossing owned boundaries.

This is the first ownership IR, not a complete borrow checker. Mutable alias exclusion, inferred reborrows, lifetime parameters, regions, destructors, ARC operations, and FFI ownership remain explicit completion gates.

## WebAssembly bootstrap backend

The direct Wasm encoder emits standard WebAssembly core modules without requiring a Rust target installation or an external assembler. User functions compile to typed Wasm functions and `main` is exported as `semaprax_main`. Contracts trap through a host import. Arithmetic lowers to a small generated JavaScript host that performs checked `i64` operations with `BigInt`, preserving the safe arithmetic semantics instead of silently accepting Wasm's wrapping operators.

The web package contains `app.wasm`, `semaprax.js`, `index.html`, `package.json`, and a `semaprax.web.v2` graph-revision/capability manifest. This is real browser-executable output; it is not yet the UI dialect, DOM renderer, SSR/hydration system, WASI target, or Component Model backend.

## Development integrity

`AGENTS.md` defines the repository invariants and change protocol. `docs/QUALITY-GATES.md` defines baseline and semantic-layer-specific evidence. CI runs formatting, strict linting, tests, release builds, native execution, Wasm instantiation, crate packaging, and the declared Rust minimum version. These gates reduce regressions; they do not turn a partial completion-matrix row into a completed one.

## Trust boundaries

- Source and patch input are untrusted and fully parsed.
- Names are restricted before reaching C identifiers.
- String data embedded into C diagnostics is escaped.
- Generated C is compiled without shell interpolation.
- Patch writes are revision-bound, verified, and atomic.
- The compiler currently invokes the host `clang`; sandboxed build execution is roadmap work.
