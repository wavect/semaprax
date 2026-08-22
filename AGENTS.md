# SEMAPRAX agent guide

SEMAPRAX is the agent-native systems programming language: **meaning in, verified machine code out**. Human-readable `.spx` source is the canonical Git projection; the versioned semantic graph is the preferred agent interface.

`docs/index.md` maps every document; `docs/SUMMARY.md` orders the same files into the published book.

## Read order

Before changing semantics, read:

1. `docs/RFC-0001.md` — language, compiler, interoperability, application, and target contract.
2. `docs/COMPLETION-MATRIX.md` — the joint full-goal audit and evidence-gated status; this prevents partial work from being described as complete.
3. `docs/ARCHITECTURE.md` — current implementation, trust boundaries, and the repository module map.
4. `docs/QUALITY-GATES.md` — required checks and change-specific evidence.
5. `docs/ROADMAP.md` — sequencing, not a reduction of the full objective.

Then read by topic before touching that topic; each document is the single source of truth for its area:

| Topic | Read first |
| --- | --- |
| Records, variants, generics, matching, `Option`, `Result` | `docs/RFC-0002-ALGEBRAIC-DATA.md` |
| Impact preview, patch provenance, reverse-call closure | `docs/SEMANTIC-IMPACT-V1.md` |
| Repair discovery/instantiation, `SPX-S103`, Patch v3 | `docs/DIAGNOSTIC-REPAIR-V1.md` |
| Read-only fixed-section review report | `docs/SEMANTIC-REVIEW-V1.md` |
| Replayable patch evidence, evidence-gated A0 route | `docs/SEMANTIC-PATCH-EVIDENCE-V1.md` |
| Compiler target projections, Evidence v2 binding | `docs/SEMANTIC-TARGET-EVIDENCE-V1.md`, `docs/SEMANTIC-PATCH-EVIDENCE-V2.md` |
| Multi-file managed publication, per-file proof carrier, evidence-gated apply | `docs/SEMANTIC-WORKSPACE-TRANSACTION-V1.md`, `docs/SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md` |
| Cross-file workspace graph, analysis, change route | `docs/SEMANTIC-WORKSPACE-V1.md`, `docs/WORKSPACE-SEMANTIC-GRAPH-V1.md`, `docs/WORKSPACE-ANALYSIS-V1.md`, `docs/SEMANTIC-WORKSPACE-CHANGE-V1.md` |
| Stable-ID multi-file rename/alias derivation | `docs/SEMANTIC-WORKSPACE-OPERATIONS-V1.md` |
| Owned-call recovery, physical failure, settlement, quiescence | `docs/RFC-0004-NATIVE-CALL-SETTLEMENT.md` (proof scaffolding, not a wired native-runtime claim) |

The `src/` module map lives only in `docs/ARCHITECTURE.md` ("Repository module map").

## Non-negotiable invariants

- A safe source program must have equivalent checked behavior on every implemented backend.
- Evaluation order is left-to-right; lazy boolean operands execute only when required.
- Public declarations have persistent `@id` identities. Expression identities are revision-scoped.
- Source formatting, graph JSON, Wasm bytes, diagnostics, and semantic patches are deterministic.
- Failed or stale semantic transactions leave source unchanged.
- A successful workspace transaction publishes one complete immutable managed
  generation through `ACTIVE`; it does not rewrite the original source files
  or grant atomic visibility to Git, editors, or other raw-path readers.
- Workspace evidence capsules have no authority. The evidence-gated workspace
  route acquires the exclusive permanent lock first, but exact replay must
  succeed before candidate generation or staging and the existing live
  Workspace invocation alone owns the `ACTIVE` pivot.
- Evidence-gated patch application acquires the ordinary A0 lock first, but
  must independently replay the exact bounded evidence before staging or
  final commit; ordinary `patch` remains the unchanged legacy route.
- Target reports and Evidence v2 capsules bind deterministic compiler-emitted
  artifacts but never claim target execution, project-test execution, safety,
  compatibility, provenance, or authority.
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
