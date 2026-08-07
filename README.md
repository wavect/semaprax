# SEMAPRAX

> **The agent-native systems programming language**
> **Meaning in. Verified machine code out.**

SEMAPRAX is an experimental programming system where source code is the human projection and a stable, queryable semantic graph is the agent interface. The v0.1 prototype accepts a small typed language, verifies its declared meaning, and lowers it to a native executable.

```text
Human source       Atomic semantic patches
     \                    /
      Versioned semantic graph
       /       |          \
  types     effects     contracts
       \       |          /
         checked core IR
                |
          C11 native lane
                |
       verified executable
```

This repository is an executable architectural seed, not a claim that the full language described in the RFC already exists. The prototype deliberately tackles the differentiator first: stable semantic identity, graph-native context, machine diagnostics, capability-aware verification, stale-safe transactions, and reproducible native output.

## Try it

Requirements: Rust 1.85+ and Clang.

```sh
cargo run -- check examples/meaning.spx
cargo run -- graph examples/meaning.spx
cargo run -- context examples/meaning.spx app.main --depth 1
cargo run -- run examples/meaning.spx
cargo run -- build examples/meaning.spx --target web -o target/meaning-web
node scripts/verify-web.mjs target/meaning-web
```

The final command compiles and runs a native executable. It prints:

```text
42
```

Install the CLI locally:

```sh
cargo install --path .
semaprax build examples/meaning.spx -o meaning
./meaning
```

## The v0.1 language

```semaprax
module examples.meaning;

@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    add(19, 23)
}
```

Implemented today:

- `i64` and `bool`, typed functions, calls, unary and binary expressions.
- Resource declarations with explicit `own`, `borrow`, and `shared` function boundaries.
- Straight-line move checking that rejects use-after-move and illegal ownership transfer.
- Checked integer arithmetic in generated programs.
- Typed `requires` and `ensures` contracts, enforced at native runtime.
- Explicit function effects checked against module capabilities and callers.
- Persistent declaration identity through `@id`.
- Deterministic formatting and graph revision hashes.
- JSON semantic graph and dependency-bounded context slices.
- JSON-line diagnostics for agent consumption.
- Atomic semantic rename patches with stale-revision rejection.
- Native AOT output through a readable C11 lowering and Clang.
- Direct WebAssembly core output with a generated ES-module runtime, HTML entry point, capability manifest, checked arithmetic, and contract traps.

Not implemented yet: lifetime and alias analysis, regions, destructors, records and variants, effect handlers, static contract proofs, Cranelift, LLVM/MLIR IR, WebAssembly Components, packages, concurrency, or cross-platform UI. Those are design commitments and staged work, not hidden behind mock commands.

## Agent protocol

Every graph response has a schema and revision:

```sh
semaprax graph examples/meaning.spx
```

An agent can request only the meaning around one symbol:

```sh
semaprax context examples/meaning.spx app.main --depth 1
```

It can then submit a transaction:

```text
base fnv1a64:<revision>
rename math.add to checked_add
require no-new-effects
```

```sh
semaprax patch examples/meaning.spx change.spatch
```

The patch updates the declaration and verified call sites together. If the graph changed since the agent observed it, SEMAPRAX returns `SPX-G409` and leaves the source untouched.

## CLI

| Command | Purpose |
| --- | --- |
| `check <file> [--json]` | Parse, type-check, verify contracts and effects |
| `graph <file>` | Emit the revisioned semantic program graph |
| `context <file> <symbol> [--depth N]` | Emit a dependency-bounded graph slice |
| `build <file> [--target native\|web] [-o path]` | Produce a native executable or deployable browser/Wasm package |
| `run <file>` | Build and run in one step |
| `fmt <file> [--check]` | Apply or verify canonical formatting |
| `patch <file> <patch.spatch>` | Apply an atomic semantic transaction |

## Why SEMAPRAX

Most coding agents edit character ranges and reconstruct meaning repeatedly. SEMAPRAX instead gives declarations persistent identity, exposes typed relationships directly, makes authority visible in signatures, and accepts changes as revision-bound semantic operations. The intended result is fewer tokens, fewer retries, and smaller trust boundaries without sacrificing readable source or Git review.

The long-term compiler has two output principles:

- Native machine code where performance and platform integration matter.
- WebAssembly Components where portability and capability sandboxing matter.

Read [RFC 0001](docs/RFC-0001.md) for the language system, [the architecture](docs/ARCHITECTURE.md) for the current implementation, [the roadmap](docs/ROADMAP.md) for the staged path forward, and the [full-goal completion matrix](docs/COMPLETION-MATRIX.md) for requirement-by-requirement evidence.

## Status

SEMAPRAX is pre-alpha research software. Its syntax, graph schema, diagnostics, and ABI will change. Do not use it for production or safety-critical workloads.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and choose an issue aligned with the current stage. Design changes should begin as an RFC because coherence is a core product property.

Licensed under Apache-2.0.
