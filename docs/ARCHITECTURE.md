# Compiler architecture

SEMAPRAX v0.1 is a vertical slice through the future compiler. Each layer has a narrow contract so the prototype can grow without turning its source syntax into its internal API.

```text
.spx source
    |
lexer -> parser -> typed AST
                    |
              semantic verifier
              /      |       \
          types    effects   contracts
                    |
             semantic graph
             /             \
      agent queries    transactions
                    |
              checked C11 IR
                    |
                  Clang
                    |
             native executable
```

## Source projection

`lexer` and `parser` accept a deliberately small grammar. `format::canonical` is the single source projection. Graph revisions hash this canonical form rather than incidental whitespace, so formatting-only edits do not invalidate an agent transaction.

Declarations should carry an explicit `@id`. Automatic identities are accepted for exploration but produce `SPX-S103`, because a name-derived ID cannot survive a rename.

## Verification

The verifier builds the module symbol table and checks:

1. Unique names and stable IDs.
2. Parameter, expression, call, and return types.
3. Boolean preconditions and postconditions.
4. Function effects against module permits.
5. Transitive effect declarations at each call edge.
6. The native entry-point shape.

The initial contract lane is progressive: contracts are type-checked at compile time and guarded in generated native code. Static proof is a later lane, and its absence is reported honestly in the project status.

Generated arithmetic is checked for overflow, zero division, and the signed division edge case. Failures have stable process exit codes and explicit diagnostics rather than C undefined behavior.

## Semantic graph

`semaprax.graph.v1` contains the canonical revision, module capabilities, stable nodes, signatures, effects, contracts, and call edges. The graph currently rebuilds per command. A later daemon will persist indexed revisions and expression identities.

Context slicing starts from a name or stable ID and walks call dependencies to a bounded depth. Callers, type references, tests, and target relationships will become additional typed edges.

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

The v0.1 backend emits readable C11, then invokes Clang. C is an implementation IR, not a promised ecosystem boundary. This gives the prototype real native binaries, easy inspection, sanitizers, and broad host support with almost no backend dependency surface.

The planned development backend is Cranelift. The planned optimizing pipeline uses multi-level IR with LLVM, while portable components lower through the WebAssembly Component Model. Backend changes must preserve the graph and verification contracts.

## Ownership seed

Resource declarations introduce non-copy semantic values. Function parameters state whether they receive ownership, borrow for the duration of a call, or participate in explicit shared ownership. The verifier evaluates the current expression language left-to-right, records moves at owned call boundaries, rejects later uses, and prevents borrowed/shared values from being returned or transferred as owned.

This is the first ownership IR, not a complete borrow checker. Control-flow joins, mutable alias exclusion, inferred reborrows, lifetimes, regions, destructors, ARC operations, and FFI ownership remain explicit completion gates.

## WebAssembly bootstrap backend

The direct Wasm encoder emits standard WebAssembly core modules without requiring a Rust target installation or an external assembler. User functions compile to typed Wasm functions and `main` is exported as `semaprax_main`. Contracts trap through a host import. Arithmetic lowers to a small generated JavaScript host that performs checked `i64` operations with `BigInt`, preserving the safe arithmetic semantics instead of silently accepting Wasm's wrapping operators.

The web package contains `app.wasm`, `semaprax.js`, `index.html`, `package.json`, and a graph-revision/capability manifest. This is real browser-executable output; it is not yet the UI dialect, DOM renderer, SSR/hydration system, WASI target, or Component Model backend.

## Trust boundaries

- Source and patch input are untrusted and fully parsed.
- Names are restricted before reaching C identifiers.
- String data embedded into C diagnostics is escaped.
- Generated C is compiled without shell interpolation.
- Patch writes are revision-bound, verified, and atomic.
- The compiler currently invokes the host `clang`; sandboxed build execution is roadmap work.
