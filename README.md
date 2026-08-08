# SEMAPRAX

> **The agent-native systems programming language**
> **Meaning in. Verified machine code out.**

SEMAPRAX is an experimental programming system where source code is the human projection and a stable, queryable semantic graph is the agent interface. The v0.2 prototype accepts a small typed language, verifies its declared meaning, and lowers it to a native executable or a deployable browser/WebAssembly package.

```text
Human source       Atomic semantic patches
     \                    /
      Versioned semantic graph
       /       |          \
  types     effects     contracts
       \       |          /
       validated stable-ID HIR
          /             \
  C11 native lane   Wasm core lane
          |             |
 native executable  browser package
```

This repository is an executable architectural seed, not a claim that the full language described in the RFC already exists. The prototype deliberately tackles the differentiator first: stable semantic identity, graph-native context, machine diagnostics, capability-aware verification, stale-safe transactions, deterministic lowering, and real native output.

## Try it

Requirements: Rust 1.85+ and Clang. Node.js 22+ is required for the shown browser/Wasm verification command.

```sh
cargo run -- check examples/meaning.spx
cargo run -- graph examples/meaning.spx
cargo run -- context examples/meaning.spx app.main --depth 1
cargo run -- run examples/meaning.spx
cargo run -- run examples/control_flow.spx
cargo run -- build examples/control_flow.spx --target web -o target/control-flow-web
node scripts/verify-web.mjs target/control-flow-web
```

The native `run` commands compile and execute host binaries; the control-flow example prints:

```text
42
```

Install the CLI locally:

```sh
cargo install --path .
semaprax build examples/meaning.spx -o meaning
./meaning
```

## The v0.2 language

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
- Resources with explicit, persistent trivial/imported lifecycles, declaration-only interface/import contracts, and `own`, `borrow`, and `shared` function boundaries.
- Lexical `let` bindings and typed `if/else` expressions.
- Control-flow-aware move checking with prefix-aware record-field state and definite or conditional use-after-move diagnostics.
- Canonical record declarations, construction, and projection in `check`, resolved HIR, and semantic Graph v6; executable targets fail closed until aggregate layout and cleanup execution land.
- A validated stable-ID HIR shared by native and Wasm lowering, with explicit entry, result, binding, expression, and place identities.
- A mandatory target-neutral cleanup CFG for every function, independently rebuilt and independently replayed against core HIR/inventory, with exhaustive current-CFG path-state checks plus a scenario-driven reference trace executor.
- Versioned target-neutral normalized-status and conformance-trace protocols, plus invocation-local immutable status arenas with zero-success/one-based tokens. The native scalar backend and narrow Wasm owned adapter use the status/out convention; complete semantic backend traces and general/native resource execution remain gated.
- Checked integer arithmetic in generated programs; native failures use exact normalized arithmetic codes and propagate without terminating an internal SEMAPRAX frame.
- Typed `requires` and `ensures` contracts, enforced by native and Wasm artifacts. Native scalar contracts publish no caller result on failure.
- Explicit function effects checked against module capabilities and callers.
- Persistent declaration identity through `@id`.
- NUL-free persistent semantic identities across source, resolved HIR, cleanup metadata, graph serialization, and native C literals.
- Deterministic formatting and domain-separated SHA-256 graph revisions.
- JSON semantic Graph v6 with persistent declaration identity, revision-scoped expression structure, complete cleanup plans, and dependency-bounded context slices.
- JSON-line diagnostics for agent consumption.
- Atomic semantic rename patches with stale-revision rejection.
- Native AOT output through a readable C11 lowering and Clang.
- Direct WebAssembly core output with a generated ES-module runtime, HTML entry point, capability manifest, checked arithmetic, and contract traps.
- A deliberately narrow `semaprax.wasm-owned.v1` Core Wasm path for one direct
  `drop trivial` resource identity. It executes validated-plan terminal cleanup,
  normalized status/out publication, and scalar or owned-input results through
  a generated instance-confined JavaScript host. The host binds its private
  ownership imports to the exact generated Wasm bytes with SHA-256 and rejects
  non-canonical ABI arguments; broader shapes remain gated.

Not implemented yet: callable native resource machine-code execution, complete
native/reference/Wasm trace conformance, the general Wasm resource ABI,
recursive reference execution, callable imports/adapters, record machine-code
layout/lowering, variants and matching, lifetime and alias analysis, user-facing
regions, effect handlers, static contract proofs, Cranelift, LLVM/MLIR IR,
WebAssembly Components, packages, concurrency, or cross-platform UI. Native
resource builds retain `SPX-B104`; Wasm admits only the documented narrow slice
and rejects every excluded resource shape with `SPX-W111`; records remain gated
with their target-specific diagnostics.

The native compiler still contains gated cleanup-CFG-driven C and trace
scaffolding plus an O0/O2/sanitizer conformance lane that compares the admitted
value corpus with the independent reference executor. Separately, the
unpublished `semaprax-native-host` crate now connects a strict pointer-free
[descriptor](docs/NATIVE-ADAPTER-DESCRIPTOR-V1.md), a real exact-instance loader
lease, the OS-seeded same-thread [capability
authority](docs/NATIVE-CAPABILITY-TOKENS-V1.md), the ownership ledger, and
non-copying owner/result wrappers. Its Linux/macOS fixtures prove descriptor
admission, trusted owner adoption, non-mutating rejection, atomic ledger
execution, generation rotation, draining, and exact lease retention. That host
does **not** resolve or call a resource function from the loaded library: the
library still exports only the descriptor getter, and test execution uses an
explicit trusted Rust closure. Windows currently supplies compile coverage, not
a native-host runtime fixture. Compiler preflight does not construct this host,
and `SPX-B104` remains unchanged.

An unpublished [native loader quarantine](docs/NATIVE-MODULE-LOADER.md) now
proves one audited unsafe trusted-library/getter/bounded-read edge and opaque
loader-reference retention with real Linux/macOS fixtures. The unpublished
native host now consumes this lease and retains it through its authority and
owners, but the loader remains disconnected from compiler emission and every
public API. It exposes no callable symbol, is not a malicious-plugin boundary,
and does not weaken `SPX-B104`.

The current critical-path implementation contract is [Owned resource vertical
slice v1](docs/OWNED-RESOURCE-VERTICAL-V1.md): one deliberately narrow,
production-reachable owned-resource corpus must execute with exact
native/reference/Wasm status, cleanup, publication, and semantic-trace equality
before either backend gate can open. The document is a gate, not a completion
claim. The narrow Wasm slice and private native host are prerequisites; they do
not yet satisfy the full cross-target contract.

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
base sha256:<64-lowercase-hex-digits>
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

Read [RFC 0001](docs/RFC-0001.md) for the language system, [RFC 0002](docs/RFC-0002-ALGEBRAIC-DATA.md) for algebraic data and aggregate ownership, and [RFC 0003](docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) for implemented lifecycle source/resolution and the proposed exactly-once cleanup/runtime phases. [Conformance trace v1](docs/CONFORMANCE-TRACE-V1.md) fixes the target-neutral status/trace projection, and [host ownership transactions v1](docs/HOST-OWNERSHIP-TRANSACTIONS-V1.md) fixes the preflight/commit/publication semantics that future ecosystem adapters must preserve. [The architecture](docs/ARCHITECTURE.md) describes the current implementation, [the quality gates](docs/QUALITY-GATES.md) define executable contribution evidence, [protocol migrations](docs/MIGRATIONS.md) cover agent-facing compatibility, [the roadmap](docs/ROADMAP.md) gives the staged path forward, and the [full-goal completion matrix](docs/COMPLETION-MATRIX.md) records requirement-by-requirement evidence.

## Status

SEMAPRAX is pre-alpha research software. Its syntax, graph schema, diagnostics, and ABI will change. Do not use it for production or safety-critical workloads.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and choose an issue aligned with the current stage. Design changes should begin as an RFC because coherence is a core product property.

Licensed under Apache-2.0.
