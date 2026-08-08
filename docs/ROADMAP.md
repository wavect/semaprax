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

Status: in progress. Resource ownership boundaries and explicit lifecycle/interface contracts, lexical `let`, typed `if/else`, partial-place diagnostics, record syntax/checking, Graph v6, validated stable-ID HIR/type facts, a mandatory replay-validated cleanup plan, versioned normalized-status/trace protocol types, native scalar status/out execution, and a browser-loadable scalar Wasm backend are implemented; the remaining gates below are not.

- Records, variants, `Option`, and `Result`.
- Exhaustive pattern matching.
- Generic functions with constraints.
- Modules, imports, and multi-file graph commits.
- First-class diagnostic repair operations.
- Property tests generated from types and contracts.
- A persistent graph daemon and JSON-RPC agent transport.
- Complete ownership/lifetime/region analysis across control flow.

Exit criterion: build a non-trivial CLI and edit it entirely through semantic transactions.

The aggregate tranche is specified in [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md). RFC 0003 phases 1–2 now supply explicit trivial/imported lifecycle syntax, declaration-only interface/import contracts, source/HIR validation, and a target-neutral cleanup plan. Resolved functions carry typed blocks/edges/regions/exits, path-sensitive guarded liveness, atomic call commits, sticky failure sources, partial-record cleanup order, whole-value normalization, and result publication; validation independently rebuilds the plan, and Graph v6 serializes it. The versioned normalized-status arena, semantic trace projection, independent inventory/HIR coverage replay, exhaustive current-CFG state replay, single-frame scenario executor, and native scalar status/out execution are implemented as phase-3 groundwork. Native resource cleanup/instrumentation, callable imports, adapter bindings, backend trace validation, Wasm status/resource execution, native/Wasm trace equivalence, aggregate layouts, variants, and matching remain subsequent work; backends fail closed on both records and resources.

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
