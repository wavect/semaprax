<div align="center">

# SEMAPRAX

### Meaning in. Verified machine code out.

An experimental, agent-native systems programming language built around a
stable semantic program graph.

[![CI](https://github.com/wavect/semaprax/actions/workflows/ci.yml/badge.svg)](https://github.com/wavect/semaprax/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.2.0-7c3aed.svg)](https://github.com/wavect/semaprax/blob/main/Cargo.toml)
[![Status](https://img.shields.io/badge/status-pre--alpha-f59e0b.svg)](#project-status)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-000000.svg?logo=rust)](https://github.com/wavect/semaprax/blob/main/Cargo.toml)
[![License](https://img.shields.io/badge/license-Apache--2.0-2563eb.svg)](LICENSE)

[Get started](#get-started) · [See what works](#project-status) ·
[Read the RFC](docs/RFC-0001.md) · [View the roadmap](docs/ROADMAP.md) ·
[Visit the project page](https://wavect.io/semaprax/)

</div>

> [!WARNING]
> SEMAPRAX is pre-alpha research software. Its language, graph schemas,
> diagnostics, and ABIs will change. Do not use it for production or
> safety-critical workloads.

Most programming tools—and most coding agents—work by editing character
ranges and repeatedly reconstructing program meaning. SEMAPRAX keeps readable
`.spx` source for humans and Git, while exposing a deterministic, versioned
semantic graph as the preferred interface for agents.

That design gives SEMAPRAX three defining properties:

| Principle | What it means |
| --- | --- |
| **Stable meaning** | Public declarations have persistent identities; types, effects, contracts, ownership, and call relationships are explicit. |
| **Verified changes** | Semantic patches are revision-bound, replayable, and stale-safe. Failed transactions leave source unchanged. |
| **Shared lowering** | Native and WebAssembly backends consume the same validated stable-ID HIR and target-neutral cleanup meaning. |

## Get started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.85 or newer
- Clang for native compilation
- Node.js 22 or newer for the browser/WebAssembly verification example

### Clone and run

```sh
git clone https://github.com/wavect/semaprax.git
cd semaprax

# Parse, type-check, and verify a program.
cargo run --locked -p semaprax -- check examples/meaning.spx

# Compile it to a host-native executable and run it.
cargo run --locked -p semaprax -- run examples/meaning.spx
```

The example prints:

```text
42
```

Install the development CLI locally if you prefer shorter commands:

```sh
cargo install --path .
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
```

### Inspect meaning, not text

```sh
# Emit the deterministic semantic graph.
cargo run --locked -p semaprax -- graph examples/meaning.spx

# Ask for a bounded semantic slice around a stable declaration ID.
cargo run --locked -p semaprax -- context \
  examples/meaning.spx app.main \
  --depth 1 --max-bytes 65536 --max-nodes 256
```

### Build for the web

```sh
cargo run --locked -p semaprax -- build \
  examples/control_flow.spx \
  --target web \
  -o target/control-flow-web

node scripts/verify-web.mjs target/control-flow-web
```

## A small SEMAPRAX program

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

`@id` gives a public declaration persistent semantic identity. Contracts are
checked and carried into the supported native and Wasm artifact lanes instead
of existing only as comments.

More examples cover [control flow](examples/control_flow.spx),
[effects](examples/effects.spx), [ownership](examples/ownership.spx),
[lifecycles](examples/lifecycle.spx), [records](examples/records.spx), and the
[native callable target](examples/native_callable.spx).

## How it fits together

```mermaid
flowchart LR
    S["Human-readable .spx source"] --> V["Parser + verifier"]
    V --> H["Validated stable-ID HIR"]
    H --> G["Versioned semantic graph"]
    G --> A["Context · impact · review"]
    P["Revision-bound semantic patch"] --> T["Replay + transaction gates"]
    G --> T
    T --> S
    H --> N["C11 / Clang"]
    H --> W["WebAssembly Core"]
    N --> NE["Native executable"]
    W --> WB["Browser package"]
```

Readable source is the canonical Git projection. The semantic graph is the
preferred agent projection. Both native and Wasm lowering begin only after the
same parser, resolver, verifier, and HIR validation path.

## Project status

**Current release line:** v0.2 · **Maturity:** pre-alpha research

The [completion matrix](docs/COMPLETION-MATRIX.md) is the source of truth for
project claims. At the current evidence milestone it records **38 Partial** and
**18 Missing** full-goal requirements. A feature is not called implemented
unless its stated gate has executable evidence; a successful narrow prototype
does not satisfy a broader product gate.

### Evidence-backed prototype surface

| Area | Where it is today |
| --- | --- |
| Language core | Typed `i64`/`bool` functions, bindings, expressions, checked arithmetic, contracts, effects, and explicit capabilities. |
| Data model | Bounded records, explicit Copy generics, Copy variants, compiler-owned `Option`/`Result`, exhaustive Copy matching, and a constrained postfix `?` slice. |
| Ownership | Move and partial-place checking, explicit lifecycle/ownership boundaries, and independently replayed target-neutral cleanup plans. General lifetime, aliasing, concurrency, and public resource execution remain open. |
| Agent interface | Deterministic Graph v10–v14, bounded context, impact preview, repair discovery, semantic review, and exact evidence replay. |
| Semantic changes | Atomic single-file patches plus bounded managed multi-file workspace transactions and replacements-only semantic workspace operations. These do not provide general Git/editor-tree atomicity. |
| Native target | C11/Clang scalar and bounded Copy-data execution. Public general resource/FFI/aggregate ABI admission remains closed. |
| Web target | WebAssembly Core plus a generated browser package for the admitted language slice. General public Component Model output remains open. |
| Applications | Private CI evidence exists for bounded desktop and mobile prototypes. Public SDKs, packaging, lifecycle breadth, and production distribution remain open. |
| Agent runtime | A bounded injected-host Rust API has hosted deterministic fake-host evidence. It is not a live provider transport, CLI agent, durable-memory system, wallet, payment, signing, or ambient-authority surface. |

The bounded public Agent Runtime v1 gate is hosted green at
Public Agent Runtime v1 is hosted GREEN at 8cf29aff8d1be3ccf74c36bc8c837f0c666ca067 (run 31591039261, 12/12 jobs, private and public deterministic fake-host gates on Ubuntu, macOS, and Windows).
Private Economic Agent v1 A+B is exact-head hosted green at fe75c38d898b71e3ed5c57411fb46d0dbd4fc34b in run 31611748969, including both Economic gates on Ubuntu, macOS, and Windows. Public Economic Agent v1 C is exact-head hosted green at 03f1f2736de23d03b298f265f93409de89a6be95 in run 31616168124 (12/12 jobs), including the private, process-termination, and public Economic gates on Ubuntu, macOS, and Windows.

Private Native Rust Interoperability v1 A+B local evidence is in progress under
the frozen scalar/static-link profile; public C and exact-head hosted promotion
remain held. See [Native Rust Interoperability v1](docs/NATIVE-RUST-INTEROP-V1.md).
Neither changes the matrix totals.

For precise evidence, boundaries, and non-claims, use these documents:

- [Full-goal completion matrix](docs/COMPLETION-MATRIX.md) — authoritative status
- [Architecture](docs/ARCHITECTURE.md) — implementation and trust boundaries
- [Quality gates](docs/QUALITY-GATES.md) — required executable evidence
- [Roadmap](docs/ROADMAP.md) — sequencing toward the full objective
- [Changelog](CHANGELOG.md) — notable repository changes

## Agent-native workflow

SEMAPRAX is designed so an agent can ask for the smallest useful semantic
slice, preview a typed change, inspect its consequences, and only then request
an atomic application.

```text
graph/context → semantic patch → impact/review → evidence replay → atomic apply
```

The current public protocols include:

- bounded forward, reverse, or bidirectional call context;
- stable-ID semantic patches with stale-revision rejection;
- read-only impact, diagnostic repair, and fixed-section review projections;
- independently replayable patch and target-evidence capsules;
- bounded immutable-generation workspace publication through an authenticated
  `ACTIVE` pivot for cooperating readers.

Start with [Agent Context v1](docs/AGENT-CONTEXT-V1.md),
[Semantic Impact v1](docs/SEMANTIC-IMPACT-V1.md),
[Semantic Review v1](docs/SEMANTIC-REVIEW-V1.md), and
[Semantic Patch Evidence v1](docs/SEMANTIC-PATCH-EVIDENCE-V1.md).

## CLI at a glance

| Command | Purpose |
| --- | --- |
| `semaprax check <file> [--json]` | Parse, type-check, and verify a source file. |
| `semaprax fmt <file> [--check]` | Apply or verify canonical formatting. |
| `semaprax run <file>` | Build and run a host-native program. |
| `semaprax build <file> [--target native\|native-callable\|web]` | Produce a native executable, bounded callable bundle, or browser/Wasm package. |
| `semaprax graph <file>` | Emit the revisioned semantic graph. |
| `semaprax context <file> <stable-id> [options]` | Emit a deterministic, bounded semantic context. |
| `semaprax impact <file> <patch.spatch> [options]` | Preview supported source consumers and reverse-call impact without writing. |
| `semaprax review <file> <patch.spatch>` | Emit the bounded semantic review report. |
| `semaprax patch <file> <patch.spatch>` | Apply a supported atomic semantic transaction. |

Run `semaprax --help` for the complete workspace, evidence, repair, and target
command surface.

## Roadmap

| Stage | Focus |
| --- | --- |
| **0.2 — current** | Useful core language, semantic graph, bounded agent context/change/review, and native/Wasm execution slices. |
| **0.3** | General ownership and lifetime safety, escape analysis, restricted unsafe code, and a fast development backend. |
| **0.4** | Components, packages, reproducible builds, provenance, and portable/native interop. |
| **0.5** | Structured concurrency, deterministic effects, applications, and platform adapters. |
| **1.0** | One maintained product proving the full cross-platform language and toolchain contract. |

This table is orientation, not a claim of completion. See the
[detailed roadmap](docs/ROADMAP.md) and [1.0 completion
contract](docs/COMPLETION-MATRIX.md#final-validation-product).

## Documentation

| Document | Read it for |
| --- | --- |
| [RFC 0001](docs/RFC-0001.md) | The language, compiler, interoperability, application, and target contract. |
| [RFC 0002](docs/RFC-0002-ALGEBRAIC-DATA.md) | Algebraic data, records, variants, matching, `Option`, and `Result`. |
| [RFC 0003](docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) | Cleanup, resource ownership, and ABI phases. |
| [RFC 0004](docs/RFC-0004-NATIVE-CALL-SETTLEMENT.md) | Proposed native owned-call recovery and settlement contract. |
| [Architecture](docs/ARCHITECTURE.md) | Compiler stages, backend boundaries, and repository map. |
| [Migrations](docs/MIGRATIONS.md) | Compatibility notes for agent-facing protocols. |

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the
[agent guide](AGENTS.md) before changing semantics. Compiler changes should
include a success case, a stable diagnostic regression, canonical round-trip
coverage, and native/Wasm equivalence when runtime meaning changes.

Run the full Unix quality gate with:

```sh
scripts/quality.sh
```

Design changes affecting syntax, graph schemas, transactions, effects,
ownership, contracts, or ABI should begin as an RFC. By participating, you
agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Citation and license

Use [CITATION.cff](CITATION.cff) for repository metadata and
[CITATION.md](CITATION.md) for evidence-specific citation guidance. Technical
claims should cite the exact commit and the repository document that supports
them.

SEMAPRAX is created and maintained by Wavect GmbH and distributed under the
[Apache License 2.0](LICENSE).
