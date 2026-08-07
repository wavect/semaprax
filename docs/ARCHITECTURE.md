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
7. The native entry-point shape.

The initial contract lane is progressive: contracts are type-checked at compile time, required to be effect-free, and guarded in generated native and Wasm code. Static proof is a later lane, and its absence is reported honestly in the project status.

Generated arithmetic is checked for overflow, zero division, and the signed division edge case. Failures have stable process exit codes and explicit diagnostics rather than C undefined behavior.

## Resolved HIR groundwork

`hir` is the fail-closed boundary between verified human syntax and future semantic consumers. It resolves resource types and calls through persistent declaration IDs, assigns deterministic structural identities to parameters, locals, expressions, and result values, and represents value references as places. Spans and display names remain diagnostic metadata rather than semantic identity.

The declaration index is the single current source of target-independent type facts: whether a type is copyable, contains resources, is sized, needs destruction, and its name-independent layout key. Generic parameter identities include their owner declaration plus index, and nominal identities include the complete resolved argument tree.

The native and Wasm emitters now consume only validated HIR for semantic lowering; their parsed-AST entry points are compatibility wrappers that resolve first. A centralized HIR validator rejects duplicate/non-canonical identities, invalid declarations and nominal types, lexical-scope violations, inconsistent expression/call types, definite or conditional resource reuse, contract transfers, undeclared or unpermitted effects, effectful contracts, invalid result bindings, and an invalid entrypoint before either backend emits an artifact.

This remains staged groundwork rather than the sole compiler IR: the current verifier still establishes meaning from parsed AST before HIR resolution. The semantic graph and both executable backends consume validated HIR. Record fields, variant payloads, partial-place availability, recursive aggregate fact computation, and target-specific aggregate layouts therefore remain RFC 0002 gates.

## Semantic graph

`semaprax.graph.v3` is serialized exclusively from validated resolved HIR. It contains the canonical human-source revision, module capabilities, entrypoint declaration ID, explicit-versus-automatic declaration identity origin, declaration nodes, typed parameter/result/value identities, effects, structural contracts, sorted declaration-ID call dependencies, and the structural body graph. Expressions expose a stable `type_id` and ownership mode; the top-level `type_facts` index supplies resolved type structure, copy/resource/size/drop facts, and deterministic layout keys without repeating them at every expression.

Integer literals are decimal JSON strings rather than JSON numbers, so JavaScript and TypeScript agents preserve every `i64` value exactly. `let` bindings expose one value identity; the enclosing statement does not reuse that ID as a second identity domain.

Expression and value IDs are deterministic but revision-scoped; only explicitly authored declaration IDs are persistent across revisions. Automatic name-derived declaration IDs remain visibly marked unstable. Spans are intentionally absent because canonical source revisions ignore whitespace while spans do not.

Context slicing starts from a display name or exact declaration ID, with exact IDs taking precedence on collisions. It walks declaration-ID call dependencies from preconditions, bodies, and postconditions to a bounded depth and includes only nominal type declarations referenced by selected functions. Every result declares a `module` or `context` view. Context views record root, depth, truncation, and frontier IDs so an omitted dependency is distinguishable from a dangling reference. Callers, tests, packages, targets, and generated artifacts will become additional typed edges.

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

The C emitter materializes subexpressions in source order before calls and operators. This is required because C does not define function-argument evaluation order, while SEMAPRAX does. Lazy boolean operators and `if` expressions lower to explicit branches, and generated local names cannot collide with source identifiers.

The planned development backend is Cranelift. The planned optimizing pipeline uses multi-level IR with LLVM, while portable components lower through the WebAssembly Component Model. Backend changes must preserve the graph and verification contracts.

## Ownership seed

Resource declarations introduce non-copy semantic values. Function parameters state whether they receive ownership, borrow for the duration of a call, or participate in explicit shared ownership. `let` transfers owned values while preserving borrowed/shared modes. The verifier evaluates left-to-right, records moves at owned boundaries, distinguishes definite from conditional moves, joins ownership state conservatively across `if` and lazy boolean control flow, and prevents borrowed/shared values from being returned or transferred as owned.

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
