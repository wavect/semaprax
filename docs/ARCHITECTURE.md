# Compiler architecture

Status: living internal implementation and trust-boundary map.

Audience: compiler contributors and reviewers.

This document owns the current implementation map, data flow, and trust
boundaries. It does not own product status, protocol details, historical
changes, or test inventories:

- current status: [completion matrix](COMPLETION-MATRIX.md);
- exact protocols and ABIs: their versioned reference documents;
- required checks: [quality gates](QUALITY-GATES.md);
- history: [changelog](../CHANGELOG.md).

SEMAPRAX v0.2 is a set of bounded vertical slices through a larger language
design. The architecture keeps human source, verified meaning, agent
projections, mutation authority, and target execution distinct.

## System shape

```text
canonical .spx source or held Project inputs
                    |
              lexer + parser
                    |
                 AST
                    |
        resolver + semantic verifier
                    |
          validated stable-ID HIR
                    |
       canonical cleanup-plan builder
                    |
          independent plan replay
             /       |       \
 semantic graph   interpreter   target lowering
      |                         /            \
 context/impact/review      native C11    Wasm Core
      |                          |             |
 evidence + transactions      Clang       JS/Node host
```

No backend bypasses source verification or validated-HIR checks. Cleanup-plan
vectors are canonical execution order and must not be sorted or repaired by a
graph projection or backend.

## Representations

### Canonical source

`src/lexer.rs`, `src/parser.rs`, and `src/ast.rs` parse human-readable source.
`src/format.rs` is the canonical source projection. Revision digests bind the
canonical bytes, not incidental whitespace.

Source is the canonical Git representation. A managed workspace publishes an
immutable generated source set for cooperating readers; it does not rewrite the
original files or grant atomic visibility to Git, editors, or arbitrary raw
path readers.

### Validated HIR

`src/verify.rs`, `src/source_verify.rs`, and `src/hir.rs` own checked meaning.
The `src/hir/` modules own validation, inspection indexes, declaration lookup,
and bounded Project linking.

HIR carries resolved identities and typed operations. A backend or report may
apply a stricter admission profile, but it may not reinterpret unresolved AST
or silently widen the verified program.

### Cleanup meaning

`src/cleanup.rs` inventories structurally owned leaves.
`src/cleanup_plan.rs` and `src/cleanup_plan/` own the target-neutral cleanup
control-flow schema, builder, validation, execution model, and independent
replay.

Current graph versions select the minimum schema needed by the admitted
feature, from legacy scalar/Result meaning through Option, aggregates,
generics, loops, byte data, command I/O, and owned-byte record matching. The
owning feature specifications define exact schema numbers and preservation
requirements; architecture depends only on monotonic, deterministic selection.

### Semantic graph

`src/graph.rs` and `src/graph_cleanup.rs` project validated program and cleanup
meaning. `src/call_index.rs`, `src/impact.rs`, `src/review.rs`,
`src/properties.rs`, and `src/hygienic.rs` build bounded read-only views over
verified representations.

A graph, report, review, or evidence capsule is descriptive data. It is not a
capability, signature, approval, or commit token.

## Compiler and execution lanes

### Interpreter

`src/interpreter.rs` evaluates an admitted verified-HIR profile with bounded
fuel and normalized runtime statuses. `src/hosted_interpreter.rs` adds the
bounded host-facing execution used by Project profiles. The interpreter is a
development and conformance lane, not a target backend or proof engine.

### Native bootstrap backend

`src/codegen.rs` owns native orchestration and admission. The
`src/codegen/native_*` modules own C11 emission, runtime statuses, aggregate
and byte-data lowering, command I/O, callable bundles, resource fixtures,
capability envelopes, conformance traces, and private host contracts.

The public executable lane emits C11 and invokes an explicitly admitted Clang.
Private callable and resource lanes are narrower host-integration evidence;
they do not establish a stable general native ABI.

### WebAssembly backend

`src/wasm.rs` and `src/wasm/` emit Core WebAssembly and generated host
carriers for admitted profiles. Scalar, selected aggregate, text, byte-data,
owned, and command-I/O paths remain separately admitted. The default product
is not a general WebAssembly Component Model runtime.

`src/wit_component.rs` and `src/wit_component/` provide default-off private
boundary evidence. They cannot be cited as public Component Model execution.

### Shared runtime status

`src/runtime_status.rs`, `src/semantic_trace.rs`, `src/conformance.rs`, and
`src/trace_path_certificate.rs` normalize failures and execution traces. The
first selected failure is sticky: cleanup cannot replace it, and result
publication occurs only after postconditions and non-result cleanup.

## Agent query and mutation architecture

### Single-file queries and changes

`src/agent_transport.rs` serves a bounded JSON-RPC loop over one checked
program. `src/patch.rs` owns the supported single-file transaction format and
A0 commit boundary. `src/repair.rs`, `src/impact.rs`, and `src/review.rs` are
read-only planners and projections.

`src/patch_evidence.rs` independently reconstructs supported evidence. The
evidence-gated apply route acquires ordinary A0 authority first, replays before
staging, and rechecks the unchanged source before commit. Ordinary `patch`
remains a separate legacy route.

### Managed workspace

`src/workspace.rs` owns immutable generations and the authenticated `ACTIVE`
pivot. `src/workspace_patch_evidence.rs` binds exact per-file child evidence
and replays it before candidate creation.

`src/semantic_workspace.rs`, `src/workspace_graph.rs`, and
`src/workspace_analysis.rs` own cross-file initialization, graph construction,
context, impact, and review. `src/semantic_workspace_change.rs` and its modules
own replacements-only evidence and publication. Operations and structural
change are separate, bounded derivation layers in
`src/semantic_workspace_operations.rs` and
`src/semantic_workspace_structural_change.rs`.

Only the live workspace invocation owns the final publication pivot. Evidence
capsules never carry reusable authority.

### Project profile and daemon

`src/project/manifest.rs` parses the bounded `semaprax.toml` profiles.
`src/project/` owns held input authority, immutable revisions, semantic
admission, linking, execution, builds, npm carriers, rename planning, and the
unpublished native Rust SDK bridge.

`src/project_transport/` and `src/bin/semapraxd.rs` retain one authenticated
Project revision for bounded requests. Read-only v2 is the default. Explicit
opt-ins add one server-derived rename and workflow; they do not add general
patch, filesystem, network, persistence, or recovery authority.

## Reports and projections

Read-only commands are implemented in focused modules such as
`src/abi_report.rs`, `src/c_header.rs`, `src/cxx_shim.rs`,
`src/capability_manifest.rs`, `src/freestanding_object.rs`, `src/openapi.rs`,
`src/package_report.rs`, `src/plugin_manifest.rs`, `src/region_report.rs`,
`src/simd_report.rs`, and `src/ui_schema.rs`.

These modules must:

- consume verified representations;
- use closed admission and exclusion vocabularies;
- emit deterministic bounded output;
- independently replay digest-authenticated envelopes where specified;
- make target execution and unsupported surfaces explicit non-claims.

A report can deepen a completion row from Missing to Partial. It cannot prove
the runtime or ecosystem feature it describes.

## Private host and proof boundaries

The following areas are deliberately quarantined from the public compiler
contract:

- `crates/semaprax-native-loader`: unsafe dynamic-loader boundary;
- `crates/semaprax-native-host`: connected callable and settlement host;
- `crates/semaprax-native-rust-interop-*`: unpublished deterministic Rust SDK
  builder and platform-specific publication authority;
- `src/native_settlement.rs`, `src/arc_zones.rs`, and `src/scoped_tasks.rs`:
  target-neutral proof models rather than wired runtime features;
- `src/agent_runtime.rs` and `src/economic_agent.rs`: injected-host Rust APIs
  with no built-in provider transport, keys, wallet, or ambient authority;
- `platform-tests/`: installed application and runtime fixtures whose claims
  count only when the owning hosted jobs are green.

Private or proof-only evidence may validate a design boundary without creating
a supported language, CLI, ABI, or runtime surface.

## Trust boundaries and invariants

1. Safe source must have equivalent checked behavior on every backend that
   claims to implement the admitted feature.
2. Evaluation order is left to right; lazy boolean operands execute only when
   required.
3. Public declaration IDs persist; expression IDs may be revision-scoped.
4. Source formatting, graph JSON, reports, diagnostics, patches, and generated
   artifacts covered by a contract are deterministic.
5. Failed or stale transactions leave authoritative source or the active
   generation unchanged.
6. Capabilities are explicit. Compiler and generated code gain no ambient
   filesystem, process, network, home, or secret authority.
7. Ownership errors are compile-time diagnostics, never backend accidents.
8. Owned calls stage arguments left to right and transfer them together at the
   declared commit boundary.
9. Proof data never authorizes a physical finalizer, build, or publication.
10. No feature is complete without the completion matrix's executable gate.

## Repository map

| Area | Primary owners |
| --- | --- |
| Source projection | `src/ast.rs`, `src/lexer.rs`, `src/parser.rs`, `src/format.rs` |
| Verification and HIR | `src/verify.rs`, `src/source_verify.rs`, `src/hir.rs`, `src/hir/` |
| Cleanup and layouts | `src/cleanup.rs`, `src/cleanup_plan.rs`, `src/cleanup_plan/`, `src/aggregate_layout.rs`, `src/variant_layout.rs` |
| Graph and read-only analysis | `src/graph.rs`, `src/graph_cleanup.rs`, `src/call_index.rs`, `src/impact.rs`, `src/review.rs` |
| Single-file transactions | `src/patch.rs`, `src/patch/`, `src/patch_evidence.rs`, `src/repair.rs` |
| Managed workspace | `src/workspace.rs`, `src/workspace_*`, `src/semantic_workspace*` |
| Project and daemon | `src/project/`, `src/project_transport/`, `src/bin/semapraxd.rs` |
| Interpreter | `src/interpreter.rs`, `src/hosted_interpreter.rs` |
| Native backend | `src/codegen.rs`, `src/codegen/native_*` |
| WebAssembly backend | `src/wasm.rs`, `src/wasm/` |
| Reports | the focused `*_report`, schema, manifest, header, and shim modules |
| Private host/runtime evidence | `crates/semaprax-native-*`, `platform-tests/` |
| Executable evidence | `tests/`, crate-local tests, `platform-tests/`, `.github/workflows/` |

This table is the single module-level map. Other contributor documents should
link here instead of copying it.
