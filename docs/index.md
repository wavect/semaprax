# SEMAPRAX documentation

SEMAPRAX is the agent-native systems programming language: **meaning in,
verified machine code out**. Human-readable `.spx` source is the canonical Git
projection; the versioned semantic graph is the preferred agent interface.

This book groups every repository document. All files stay at their stable
paths under `docs/`; this page is the map, `SUMMARY.md` is the table of
contents.

## Start by role

| Role | Read |
| --- | --- |
| New to the project | Root [README](../README.md), then [RFC 0001](RFC-0001.md). |
| Changing the compiler | [AGENTS.md](../AGENTS.md) first, then completion matrix, architecture, quality gates below. |
| Coding agent | [AGENTS.md](../AGENTS.md) is the entry contract; use `semaprax graph` / `context` before reconstructing meaning from source text. |
| Integrating a target | Parts *Targets and backends* and *Platform adapters*. |

## Conventions

- Every specification opens with a `Status:` line; claims are evidence-gated
  per the [quality gates](QUALITY-GATES.md).
- The [completion matrix](COMPLETION-MATRIX.md) is authoritative for what is
  implemented; nothing counts as done without its executable gate.
- Protocol compatibility changes are recorded in
  [protocol migrations](MIGRATIONS.md).

## Document map

| Document | Purpose |
| --- | --- |
| **Foundations** | |
| [RFC 0001](RFC-0001.md) | Language, compiler, interop, application, target contract. |
| [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) | Records, variants, generics, matching, `Option`, `Result`. |
| [Owned Byte Record Algebra v1](OWNED-BYTE-RECORD-ALGEBRA-V1.md) | Flat internal owned-Bytes records and ownership-aware matching. |
| [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) | Cleanup, ownership, resource ABI phases. |
| [RFC 0004](RFC-0004-NATIVE-CALL-SETTLEMENT.md) | Proposed native owned-call recovery/settlement (proof scaffolding). |
| **Status and process** | |
| [Completion matrix](COMPLETION-MATRIX.md) | Authoritative evidence-gated implementation status. |
| [Roadmap](ROADMAP.md) | Sequencing toward the full objective. |
| [Architecture](ARCHITECTURE.md) | Compiler stages, trust boundaries, repository module map. |
| [Quality gates](QUALITY-GATES.md) | Required checks and change-specific evidence. |
| [Migrations](MIGRATIONS.md) | Compatibility notes for agent-facing protocols. |
| [Decision 0001](decisions/0001-graphify.md) | Graph-first navigation policy. |
| [Decision 0002](decisions/0002-managed-workspace-generations.md) | Managed workspace generations. |
| **Agent interface** | |
| [Agent context v1](AGENT-CONTEXT-V1.md) | Default bounded context CLI projection. |
| [Agent context v2](AGENT-CONTEXT-V2.md) | Directional/filtered context add-on. |
| [Semantic impact](SEMANTIC-IMPACT-V1.md) | Read-only impact preview, reverse-call closure. |
| [Semantic patch v2](SEMANTIC-PATCH-V2.md) | Identity-scoped single-file transaction format. |
| [Diagnostic repair](DIAGNOSTIC-REPAIR-V1.md) | Repair discovery/instantiation, Patch v3 identity rebase. |
| [Semantic review](SEMANTIC-REVIEW-V1.md) | Read-only fixed-section review report. |
| [Patch evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md) | Replayable patch evidence, evidence-gated apply. |
| [Patch evidence v2](SEMANTIC-PATCH-EVIDENCE-V2.md) | Evidence binding for compiler-emitted artifacts. |
| [Target evidence](SEMANTIC-TARGET-EVIDENCE-V1.md) | Deterministic compiler target projection reports. |
| [Property tests](PROPERTY-TESTS-V1.md) | Deterministic property generation over verified sources. |
| [Conformance trace](CONFORMANCE-TRACE-V1.md) | Cross-backend execution trace format. |
| [Agent transport](AGENT-TRANSPORT-V1.md) | JSON-RPC loop over one checked program (`serve`). |
| [Agent runtime](AGENT-RUNTIME-V1.md) | Injected-host agent profile, Trace, Evidence. |
| [Context economics](AGENT-ECONOMICS-V1.md) | Offline context benchmark manifest/format. |
| [Economic agent](ECONOMIC-AGENT-V1.md) | Test-network payment intent/custody policy core. |
| **Semantic workspace** | |
| [Workspace overview](SEMANTIC-WORKSPACE-V1.md) | Additive cross-file workspace initialization. |
| [Workspace graph](WORKSPACE-SEMANTIC-GRAPH-V1.md) | Unified authenticated cross-file graph. |
| [Workspace analysis](WORKSPACE-ANALYSIS-V1.md) | Bounded Context/Impact/Review across files. |
| [Workspace change](SEMANTIC-WORKSPACE-CHANGE-V1.md) | Replacements-only evidence-gated change route. |
| [Workspace operations](SEMANTIC-WORKSPACE-OPERATIONS-V1.md) | Stable-ID rename/alias derivation. |
| [Workspace transactions](SEMANTIC-WORKSPACE-TRANSACTION-V1.md) | Managed immutable-generation publication. |
| [Workspace patch evidence](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) | Per-file proof carrier, replay-before-apply. |
| **Targets and backends** | |
| [Project manifest](PROJECT-MANIFEST-V1.md) | Bounded scalar/Web `semaprax.toml` build input. |
| [Wasm scalar exports](WASM-SCALAR-EXPORTS-V1.md) | Selected stable-ID scalar JS bindings. |
| [Wasm owned ABI](WASM-OWNED-ABI-V1.md) | Owned-value WebAssembly boundary. |
| [WIT component boundary](WIT-COMPONENT-BOUNDARY-V1.md) | Default-off WIT/schema boundary evidence. |
| [Native callable ABI v2](NATIVE-CALLABLE-ABI-V2.md) | Private callable bundle ABI. |
| [Native callable ABI v3](NATIVE-CALLABLE-ABI-V3.md) | Owned-identity callable ABI with settlement. |
| [Callable settlement proof](NATIVE-CALLABLE-SETTLEMENT-PROOF-V1.md) | Settlement-model proof corpus. |
| [Capability tokens](NATIVE-CAPABILITY-TOKENS-V1.md) | Explicit host capability grants. |
| [Owned resource slice](OWNED-RESOURCE-VERTICAL-V1.md) | First public resource-execution contract. |
| [Module loader quarantine](NATIVE-MODULE-LOADER.md) | Unsafe loader isolation boundary. |
| [Rust-host sanitizers](RUST-HOST-SANITIZERS.md) | ASan/UBSan evidence lanes. |
| [Native Rust interop](NATIVE-RUST-INTEROP-V1.md) | Generated Rust SDK package lane. |
| **Platform adapters** | |
| [Desktop app](DESKTOP-NATIVE-APP-V1.md) | Private macOS/Windows packaging path. |
| [Desktop UI](DESKTOP-NATIVE-UI-V1.md) | Private AppKit/Win32 frontend fixtures. |
| [Swift ownership](APPLE-SWIFT-OWNERSHIP-V1.md) | Apple adapter ownership mapping. |
| [Android JNI ownership](ANDROID-JNI-OWNERSHIP-V1.md) | JNI adapter ownership mapping. |
| [Host ownership txns](HOST-OWNERSHIP-TRANSACTIONS-V1.md) | Host-side ownership transaction rules. |
| [Adapter descriptor](NATIVE-ADAPTER-DESCRIPTOR-V1.md) | C descriptor contract for adapters. |
