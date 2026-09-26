# Kernel-0 Rung-2 Owned Handoff v1

Audience: compiler and self-hosting contributors.

Status: private implementation for R16 / #294 with the bounded local evidence
recorded below. This is not accepted-head, hosted, full-gate, or self-hosting-rung
evidence. Remaining acceptance gates and the exact accepted revision must be
recorded independently.

## Closed subject and proof boundary

The five renderer sources, grammar, evaluator, translation, and Lean theorem
are unchanged. They compute bytes before Rust assembles them; neither step is
an owned renderer or an ownership theorem.

The candidate then passes those authenticated bytes through
`kernel-zero.owned-handoff.move`, a checked `own Bytes -> Bytes` function with
one immutable move binding. Normal source/HIR ownership verification and
canonical cleanup replay govern it. Rust remains formatter authority: only a
fully settled Rust-equal candidate may enter the caller's 20-byte token.

## Synchronous admission and settlement

`interpreter::retained_call::owned_handoff` owns a crate-private sealed product
with immutable checked HIR and its prepared call. Before ordinary verification,
admission caps the inventory at three functions, forbids authored types, generic
templates/instances, contracts, effects and yields, and checks at most 32 nodes
and depth four per body. The selected function has one owned Bytes parameter,
one immutable owned move binding, a plain owned place result, and no callees.
Other admitted nodes exist only for the target-input shim and scalar main.
The unreferenced implicit Option/Result prelude is retained and independently
validated normally; its presence admits no aggregate handoff.

Input length is `0..=20`; fuel is `1..=8`. Violations refuse before owner
staging. Evaluation recursion is just the selected block and plain places,
not the ordinary interpreter's 256-frame profile. No worker or process is
created. The existing general retained-call API keeps its 64 MiB worker stack
and shares the factored staging, call frame, failure mapping, and harvesting.

Staging creates one ordinary interpreter byte owner, even for empty input.
A weak reference first observes its unique live strong owner. Only after
execution and harvesting release the last strong owner can a settlement
receipt exist. Publication also requires exactly the existing
`CopyOutAndSettleBytes` event, the expected length, and unchanged bytes. The
weak reference grants no cleanup authority and cannot drop an owner.
Exhaustion publishes nothing and must release the owner without inventing a
successful result-harvest event. Unwind panics also release staged ownership;
this is not recovery from allocator abort, stack overflow, or fail-stop.

## Exact private binding and target evidence

`kernel_zero::rung_two_owned_handoff` owns the adapter and bounded binding.
Initialization checks the embedded wrapper, entry, five unchanged scalar
sources, each exact entry and canonical core term, 20-byte maximum, descriptor,
native C provider bytes, Core-Wasm bytes, and Cargo.lock. It reuses the existing
canonical term encoder and source/entry inventory without changing bootstrap-v2 bytes. Only
the tiny wrapper target artifacts are emitted during initialization, not the
five scalar targets; no target compilation or execution occurs there.

Descriptor/emitter APIs receive a private standalone-source subject bound to
source, entry and profile digests: not a managed Project generation or release
provenance. The target-only borrowed-input shim copies its input and calls the
same owned handoff. These are wrapper-boundary artifacts, not owned Kernel
theorems. The existing public owned-data ABI keeps its own larger limit; the
private evidence driver admits at most 20 bytes before calling that ABI.

Before owner allocation the adapter checks artifact length, digest, and complete
equality with its held compiler-derived snapshot. This closed expected-byte
verifier does not parse arbitrary artifact members. Reminting a digest cannot
authorize a changed source, entry, source order, core entry, core term,
descriptor, target, or maximum. Bootstrap-v2's separate decoder and hostile
gates are unchanged.

## Recovery and required evidence

One panic-safe thread-local scope covers scalar execution and handoff. Nested
formatting and active bounded-output scopes bypass both. Refusal, binding drift,
exhaustion, missing settlement, mismatch or unwind panic selects the original
Rust fallback; a later ordinary invocation may re-enter. Production formatting
runs no target executable and gains no filesystem, network, process,
persistence, or worker-thread authority.

The `owned_handoff` library selector covers real owner lifetime, async/synchronous
equality, 0/1/20-byte values, capacity-plus-one, exhaustion, missing receipts,
shallow admission, panic, and a two-MiB test stack. A test-only retained strong
alias makes last-owner release fail and prevents result publication; releasing
that fixture alias restores subsequent success. Graph replay pins the owned
parameter/result and live cleanup entry, and rejects a forged borrow mode.
Reminted substitutions must stage zero owners. The normal formatter's five-lane
gate additionally requires one successful settled handoff per candidate traversal,
so Rust-only fallback
cannot satisfy it.

Physical wrapper evidence requires C11 O0/O2 and Node/Core-Wasm with
`SEMAPRAX_REQUIRE_KERNEL_ZERO_RUNG_TWO_TARGETS=1`. Thirteen rows cover all five
lanes, non-palindromic tokens, empty/NUL/non-UTF-8 bytes, and `i64::MIN`'s exact
20-byte output. Allocator/arena observations cover lifetime, copy refusal,
stale/double-drop refusal, settlement and re-entry. The private host arena is a
bounded test witness, not evidence of generated npm or browser execution.
Native free-call observations include one `free(NULL)` for empty Bytes;
non-NULL live allocations are counted separately. Stale drop must add neither
a free call nor a live owner. Thirteen rows passed locally for each of native
O0, native O2 and Core-Wasm, but this does not replace scalar-target gates.

Existing authority, broad renderer, bootstrap reproducibility/hostility,
scalar real-target recovery, differential, and required Lean gates must pass
at the accepted exact revision. Hosted acceptance and any change from rung 1
remain explicit independent review decisions. This profile promotes no rung,
public ABI, support policy, whole-compiler self-hosting or verification claim.

## Rung criteria and proof assumptions mapped to evidence

Issue #294 asks for the existing Kernel-0 specification/proof assumptions and
the self-hosting-rung ladder (`docs/SEMANTIC-KERNEL-V1.md`, "Self-hosting gate
ladder") to be mapped against current executable evidence, without rebuilding
any already-proved or already-reference piece. This table is that map; it
records status only, and promotes nothing by itself.

| # | Assumption / criterion | Status | Evidence |
|---|---|---|---|
| 1 | Kernel-0 type safety (Progress, Preservation) over the whole grammar, including `Let` and non-recursive `Call` | **Proved** (Lean 4, zero `sorry`/`admit`/custom axiom; `#print axioms` reports only `propext`/`Quot.sound`) | `docs/KERNEL-PROOF-MECHANIZATION-V1.md`; `docs/SEMANTIC-KERNEL-V1.md` "Paper safety proof". Wired into `quality.sh full` as `kernel0-lean-proof-gate`; a **hosted** verdict is still pending (not re-run this slice; no Actions credits). |
| 2 | HIR-to-Kernel-0 reification predicate is real, mechanically checked code, not prose | **Tested** | `src/kernel_zero.rs` (`reifies_into_kernel_zero`) and its `tests` submodule. Deliberately inert: it narrows nothing it does not already reject. |
| 3 | Reference interpreter agrees with the compiler's interpreter over a finite corpus | **Tested**, partial (finite corpus, not a proof) | `kernel_zero::differential::reference_interpreter_agrees_with_the_compiler_over_the_kernel_zero_corpus`; this slice reran it to completion (see "Local implementation evidence"). |
| 4 | Native C11 (`-O0`/`-O2`) and Core Wasm agree with the reference interpreter over the same corpus | **Tested**, partial (finite corpus, not a proof) | `kernel_zero::differential::cross_backend::native_c11_and_core_wasm_agree_with_the_kernel_zero_reference_interpreter_over_the_corpus`; this slice reran it to completion. |
| 5 | Rung 0 (one concrete program, same result on interpreter/native/Wasm) | **Reached** | `docs/SEMANTIC-KERNEL-V1.md` "Rung 0 evidence". Not rebuilt here. |
| 6 | Rung 1 (kernel-sized pure computation, cross-backend agreement) | **Reached** | `rung_one_capacity_classifier_reifies_and_matches_reference_and_compiler_interpreters` and `...cross_backend::rung_one_capacity_classifier_agrees_across_native_o0_o2_and_core_wasm` (72 fixtures, 216 native/Wasm comparisons). Not rebuilt here. |
| 7 | Rung 2 (self-hosted, pure, ownership/effect-free compiler component; broad differential; bootstrap-reproducible) | **Not reached** | `docs/SEMANTIC-KERNEL-V1.md` ladder still records "No" for rung 2. The five renderer lanes (`kernel_zero::rung_two_renderer::`, 14 passed) and the bootstrap artifact (`kernel_zero::rung_two_bootstrap::`, 10 passed) are local target/recovery evidence toward it, not the rung itself; this slice does not flip that status. |
| 8 | Owned `Bytes` boundary: exact source/term/target binding is authenticated, and no owner is allocated before that check | **Tested** | `rung_two_owned_handoff/binding.rs` (`Binding::authenticate`) and `rung_two_owned_handoff/tests.rs::reminted_owned_handoff_substitutions_refuse_before_owner_allocation` — 9 independent mutation classes (source, entry, maximum, core source order, core entry, core term, native target, Wasm target, descriptor) plus truncated/legacy bytes all refuse with zero owners staged. |
| 9 | Owned `Bytes` boundary: bounded execution (admission caps, 0..=20-byte input, 1..=8 fuel) | **Tested** | `interpreter::retained_call::owned_handoff` + its `tests` module (`owned_handoff` selector, 10 passed). |
| 10 | Owned `Bytes` boundary: a refused, exhausted, mismatched, or panicking candidate falls back to the authoritative Rust bytes, and a later invocation re-enters without double-applying effects | **Tested** | `tests.rs::owned_handoff_exhaustion_has_no_candidate_and_next_invocation_recovers` (fuel-exhaustion refusal, then a fresh successful call); `tests.rs::reminted_owned_handoff_substitutions_refuse_before_owner_allocation`'s final assertion (re-entry with `b"reentry"` after every mutation refusal); `targets.rs` native/Wasm hostile handle/copy-refusal-then-reentry rows. |
| 11 | Owned `Bytes` boundary: physical native (`-O0`/`-O2`) and Core-Wasm execution over real allocator/arena lifetimes | **Tested** | `targets.rs::owned_handoff_native_and_wasm_settle_refuse_and_reenter`, 13 rows (empty/NUL/non-UTF-8 bytes, `i64::MIN`'s 20-byte output, all five renderer lanes); this slice reran it (see below). |
| 12 | The binding-authentication check in criterion 8 is load-bearing, not incidental | **Demonstrated once, reverted** (negative control) | This slice's negative-control mutant; see "Negative-control mutant" below. Not committed. |
| 13 | Rung-2 promotion / owned-buffer formatter authority transfer | **Assumed open**, explicitly not decided here | Per issue #294's own boundary: "Rung promotion itself stays an explicit reviewed decision." This slice states evidence, not a promotion. |
| 14 | Hosted acceptance (CI-run Lean gate, hosted differential, release-blocker set) | **Open** | Actions credits exhausted this session (see `docs/DEVELOPMENT.md`/coordinator notes); no hosted claim is made anywhere in this document. |
| 15 | Whole-compiler self-hosting or formal verification (issue #212) | **Out of scope** | Not imported into this slice; #294 explicitly excludes it. |

## Local implementation evidence

The following selectors completed against the local implementation before its
review commit, using a worktree-private target directory, one Cargo build job,
disabled incremental compilation/debug information, and one test thread:

- `owned_handoff`: 10 passed, including required physical wrapper targets.
- `kernel_zero::rung_two_authority::`: 8 passed.
- `kernel_zero::rung_two_renderer::`: 14 passed.
- `kernel_zero::rung_two_bootstrap::`: 10 passed with required native/Wasm targets.

The four-test `kernel_zero::differential::` selector is **incomplete**. Its first
two tests passed: the 2,562-comparison C11 O0/O2 and Core-Wasm corpus reported no
disagreements, and the rung-one capacity-classifier target test passed. The run
was terminated at the user's direction while the third test was active; the
fourth test was not reached. These partial observations are not an overall
differential pass. Lean/formal and full quality gates were not run for this
slice. No full-gate or accepted-revision conclusion follows from these local
results, and further broad runs are outside the user's authorized scope.
