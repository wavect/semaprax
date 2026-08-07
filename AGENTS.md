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

## Repository map

- `src/ast.rs`, `lexer.rs`, `parser.rs`, `format.rs`: human source projection.
- `src/verify.rs`, `src/hir.rs`: checked semantics and the stable-ID resolved representation.
- `src/cleanup.rs`: structural cleanup storage/leaf inventory.
- `src/cleanup_plan.rs`, `src/cleanup_plan/`: target-neutral cleanup CFG schema, canonical builder, and independent replay gate.
- `src/graph_cleanup.rs`: deterministic tagged Graph v6 projection of validated cleanup plans.
- `src/graph.rs`, `patch.rs`: agent representation and atomic transactions.
- `src/codegen.rs`, `wasm.rs`: native C11/Clang and browser/Wasm bootstrap lanes.
- `tests/`: executable language, graph, transaction, ownership, and backend evidence.
- `examples/`: canonical programs exercised directly in CI.

## Non-negotiable invariants

- A safe source program must have equivalent checked behavior on every implemented backend.
- Evaluation order is left-to-right; lazy boolean operands execute only when required.
- Public declarations have persistent `@id` identities. Expression identities are revision-scoped.
- Source formatting, graph JSON, Wasm bytes, diagnostics, and semantic patches are deterministic.
- Failed or stale semantic transactions leave source unchanged.
- Capabilities are explicit; compiler and generated code gain no ambient authority silently.
- Ownership errors are compile-time diagnostics, never backend accidents.
- Cleanup inventory discovery order is structural metadata, never runtime liveness or destruction order.
- Cleanup-plan vectors are canonical execution order and must never be sorted or repaired by Graph/backends.
- An owned call stages every argument left-to-right in caller-owned epochs and transfers all of them at one atomic `CallCommit`.
- Failure selection is sticky; cleanup cannot replace its status, and result publication occurs only after postconditions and non-result cleanup.
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
