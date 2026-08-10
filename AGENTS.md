# SEMAPRAX agent guide

SEMAPRAX is the agent-native systems programming language: **meaning in, verified machine code out**. Human-readable `.spx` source is the canonical Git projection; the versioned semantic graph is the preferred agent interface.

## Read order

Before changing semantics, read:

1. `docs/RFC-0001.md` — language, compiler, interoperability, application, and target contract.
2. `docs/COMPLETION-MATRIX.md` — the joint full-goal audit and evidence-gated status; this prevents partial work from being described as complete.
3. `docs/ARCHITECTURE.md` — current implementation and trust boundaries.
4. `docs/QUALITY-GATES.md` — required checks and change-specific evidence.
5. `docs/ROADMAP.md` — sequencing, not a reduction of the full objective.

For records, variants, generics, matching, `Option`, or `Result`, also read `docs/RFC-0002-ALGEBRAIC-DATA.md` before editing.
For semantic-impact preview, patch provenance, source-consumer facts, or
reverse-call closure, also read `docs/SEMANTIC-IMPACT-V1.md`.
For diagnostic repair discovery/instantiation, `SPX-S103` identity assignment,
or Semantic Patch v3, also read `docs/DIAGNOSTIC-REPAIR-V1.md`.
For the fixed-section read-only Patch v1/v2/v3 review report, also read
`docs/SEMANTIC-REVIEW-V1.md`.
For native owned-call recovery, physical failure, settlement, or quiescence,
also read `docs/RFC-0004-NATIVE-CALL-SETTLEMENT.md`; its hidden Rust model is
proof scaffolding, not a wired native-runtime claim.

## Repository map

- `src/ast.rs`, `lexer.rs`, `parser.rs`, `format.rs`: human source projection.
- `src/verify.rs`, `src/hir.rs`: checked semantics and the stable-ID resolved representation.
- `src/cleanup.rs`: structural cleanup storage/leaf inventory.
- `src/cleanup_plan.rs`, `src/cleanup_plan/`: target-neutral cleanup CFG schema, canonical builder, and independent replay gate.
- `src/aggregate_layout.rs`, `src/variant_layout.rs`: checked deterministic Native64/Wasm32 internal layouts for the admitted record and copy-variant field kinds.
- `src/trace_path_certificate.rs`: canonical compiler-owned cleanup trace trie-DFA and outcome certificate.
- `src/native_settlement.rs`: hidden target-neutral callable-v3 settlement model; no loader, host, provider, or public backend wiring.
- `src/graph_cleanup.rs`: deterministic tagged cleanup projection inside the
  program-level Graph v10/v11/v12/v13/v14 lattice; bounded generic function
  declarations select v14 above authenticated explicit Copy-record patterns at
  v13, while CleanupPlan v2 remains canonical unless authenticated Option
  propagation requires v3.
- `src/graph.rs`, `patch.rs`: agent representation and atomic transactions.
- `src/call_index.rs`, `impact.rs`: shared validated-HIR call index and bounded,
  read-only single-file Semantic Impact v1 preview.
- `src/repair.rs`: bounded read-only Diagnostic Repair v1 discovery and
  instantiation plus the independently replayed Patch-v3 identity-rebase gate.
- `src/review.rs`: bounded read-only Semantic Review v1 over complete Impact-v1
  or shared identity-rebase evidence.
- `src/codegen.rs`, `src/codegen/native_callable_*`, `wasm.rs`: native C11/Clang, private callable-v2, and browser/Wasm lanes.
- `src/wit_component.rs`: default-off deterministic WIT/schema/JavaScript boundary evidence; not a Component Model runtime.
- `crates/semaprax-native-loader`, `crates/semaprax-native-host`: unpublished unsafe loader quarantine and connected callable authority/ledger host.
- `platform-tests/`: private installed-app/native-process packaging and runtime gates; claims count only after their hosted jobs are green.
- `tests/`: executable language, graph, transaction, ownership, and backend evidence.
- `examples/`: canonical programs exercised directly in CI.

## Non-negotiable invariants

- A safe source program must have equivalent checked behavior on every implemented backend.
- Evaluation order is left-to-right; lazy boolean operands execute only when required.
- Public declarations have persistent `@id` identities. Expression identities are revision-scoped.
- Source formatting, graph JSON, Wasm bytes, diagnostics, and semantic patches are deterministic.
- Failed or stale semantic transactions leave source unchanged.
- Semantic impact preview is read-only, digest-bound to its processed patch
  bytes, and fail-closed on source snapshot drift.
- Diagnostic-repair discovery and instantiation are read-only. Semantic Patch
  v3 commit authority is limited to one canonical `assign-function-id`
  operation whose complete `breaking_identity_rebase` is independently
  revalidated before unchanged A0 commit.
- Capabilities are explicit; compiler and generated code gain no ambient authority silently.
- Ownership errors are compile-time diagnostics, never backend accidents.
- Cleanup inventory discovery order is structural metadata, never runtime liveness or destruction order.
- Cleanup-plan vectors are canonical execution order and must never be sorted or repaired by Graph/backends.
- An owned call stages every argument left-to-right in caller-owned epochs and transfers all of them at one atomic `CallCommit`.
- Failure selection is sticky; cleanup cannot replace its status, and result publication occurs only after postconditions and non-result cleanup.
- A settlement-model action is proof data, not permission to perform a physical finalizer; only future exact-instance host admission may own that authority.
- No feature is “implemented” without the completion gate’s executable evidence.

## Change protocol

1. Identify affected completion-matrix rows and invariants.
2. Add a success case and a stable diagnostic regression before or with the implementation.
3. Update parser, canonical formatter, resolver/HIR, verifier, semantic graph, native backend, and Wasm backend together when syntax carries runtime meaning.
4. Exercise agent and human projections: canonical round-trip plus graph assertions.
5. Run `scripts/quality.sh` on Unix, or the commands in `docs/QUALITY-GATES.md` on any host.
6. Update architecture, roadmap, changelog, and completion evidence honestly.

Use `cargo run -- graph <file>` and `cargo run -- context <file> <stable-id> --depth N` before reconstructing SEMAPRAX meaning from source text. Use bounded repository tooling such as lean-ctx for Rust/platform-host navigation. See `docs/decisions/0001-graphify.md` before adding another repository graph index.

Do not edit generated files under `target/`, commit tool caches, introduce build-time network access, bypass verification in a backend, or weaken a test merely to make a gate green.
