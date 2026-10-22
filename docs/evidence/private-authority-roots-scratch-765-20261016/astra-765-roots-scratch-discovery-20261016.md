# Next optimization: reuse private-authority roots backing

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Discovery only, 2026-10-16. Observed ROOT `787f84774e935833cd0d8de17c6c8da21d7b4986` at `/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang`; exact inspected source hashes are in adjacent `astra-765-roots-scratch-discovery-20261016-source.json`. Paths below are relative to that root. Graft markdown cards preceded bounded current-source reads. No compiler/repository edits, Cargo, tests, builds, Claude, or runtime measurements.

**Recommend one next batch: retain and lease the private-authority roots Vec across sequential consumers, while regenerating its contents and work ledger on every use.** This is a bounded extension of #765, independent of #847. It removes real repeated storage acquisition without caching authority or changing the traversal.

The a82e344 logs establish cumulative 64MiB refusals, not causality or dominance: typed ShiftSim/Catalog/Normalizer accepted forecasts but refused during HIR final validation; callable512 requested 70,176 with 68,511 remaining (`owning-selected-08/09/10/11/31.log`). Private-authority savings can help subsequent modules only if acquisition already occurred before their refusal. They cannot be assumed to fix a first-module HIR refusal. Do not subtract these small final deficits from an assumed total remaining workload.

## Exact opportunity and overlap

#765 already retains the authenticated top-level view/legacy map and DFS frames in `authority_invocation.rs:21–32,87–165`. Do not propose that optimization again. #764's forecast work is already authored (`CHANGELOG.md:647–649`) and is not a remedy for these proven live-ledger refusals.

The residual is `authority.rs:152–204`: each successful `finish_ready` reserves `count * size_of::<(&str,&str)>()`, allocates an exact-capacity Vec, then visits roots and deduplicates them in encounter order. `count` is the complete source type census, not the number ultimately reached. A `PrivateTypes` owns this Vec until all its `contains` or signature checks finish (`:14–17,205–250`). Repeating the same caller repeats allocation even though the invocation and census are unchanged.

The existing owning test explicitly models this: `authority_invocation/tests.rs:154–196` executes sixteen derivations and asserts `frames + 16 * roots`, seventeen allocations and zero reallocations. These are authored test assertions, not new execution evidence.

Four live consumer sites require integration:

- Compact declaration recording: `validation.rs:186–192`, with `contains` at `:263–280`; keep the lease through the complete declaration loop and final compiler-map equality, ending at `:308`.
- Compact signature validation: `validation.rs:404–426`.
- Ordinary/full-HIR signature validation: `validation.rs:804–823`.
- Full declaration reconstruction: `workspace_graph.rs:5286–5292`, with `contains` at `:5348`; retain the lease through that module's complete declaration validation.

Invocation construction is at `workspace_graph.rs:4000–4002,5248–5250`. #847 changes function presentation and alias spelling; this candidate keeps the exact original Program/authored-map subjects and has no FunctionRef dependency.

## Concrete ownership design

Add an invocation-owned roots scratch Vec to the existing state, initially empty, plus a compiler-private **scoped** operation such as `with_derived_bound(caller, programs, authored, callback)`. Keep the complete owning standalone route and a separately bound predecessor oracle. Avoid a self-referential return value or detached ticket.

The scoped operation holds the state borrow through derivation **and all callback use**. A private `PrivateTypesView<'lease,'source>` contains only a root slice borrowed for the lease and a fresh `Cell<usize>` remaining-work value. Its only authority queries are the existing contains/signature operations; they cannot expose mutable Vec access or transfer a lease. The callback result cannot borrow the lease. The four current call sites are sequential and do not derive recursively. Preserve the owned API where simultaneous independent results are needed; do not globally introduce a new reentrancy restriction.

On every invocation:

1. Preserve exact programs/map pointer checks, caller pointer membership, existing sticky check, `WORK` initialization, import debit, `possible`, lazy TopLevel proof/legacy choice and `explicit_only`, in their current order (`authority_invocation.rs:49–145`). Absence must not acquire roots backing.
2. Keep existing DFS-frame acquisition first. At the old roots-allocation frontier only, acquire roots backing once at the unchanged full `count`, checked arithmetic and exact-capacity validation. For count zero use the existing empty representation; no arbitrary minimum capacity or readiness bank is needed.
3. Require an empty roots Vec, then execute the unchanged root walk and deduplication callback. Preserve every duplicate comparison debit, push order, caller-specific private predicate, depth/leaf/work cap and refusal. No sorting, census prepass, smaller guessed allocation, root-set reuse, or cached HIR signature result.
4. Lend the completed slice and fresh remaining-work cell to the complete consumer. Its `contains` calls retain their order and debit; signature callbacks compare current HIR every time. Clear roots and DFS frames on normal return and every error; a scoped guard should also clear on unwind. Drop releases the actual backing once when the invocation ends.

Retain the old distinction between derivation failure (poisons State) and a consumer's semantic mismatch. Do not accidentally cache success or poison the invocation for a previously non-poisoning semantic error. Live budget refusal remains sticky independently. Every reuse boundary must observe that sticky state before lending or continuing work; no reserve-and-continue path.

## Funding and ordinary cost

Prepay the new persistent Vec header and any explicit state/control fields **before Invocation formation** at both live construction sites; include caller/callee parameters, return/Result receivers and any new fallible constructor plumbing. Each scoped use must fund its actual guard, view, Cell, callback/return receivers, reborrows, empty/capacity branches, acquisition arguments and cleanup controls before formation. Use concrete `size_of`/checked Layout expressions at the lexical owner, not a flat control allowance or an inferred old credit. First roots acquisition still pays its complete backing; excessive capacity/failure must settle and drop correctly.

The removed debit is only a replacement roots allocation that no longer occurs. Existing DFS funding and TopLevel proof receive no second savings credit. With no private roots there is still added owner-header/control cost. With one roots use there is no backing saving. Repeated uses may save backing acquisition, but retain that backing between modules, increasing its overlap with later HIR and potentially physical highwater. Authored/import-alias and ordinary/fallback routes all require disclosure. No universal speedup, cap adjustment, refund or promised application admission follows.

## Owning acceptance

Preserve and execute the complete prechange invocation/PrivateTypes receiver and its original sixteen-derivation assertions independently; add current tests under the same existing harness. Compare exact root order **and pointer subjects**, remaining WORK after derivation/contains, callback counts, and all six diagnostic fields/order on complete four-route consumers.

Required cases: alternating callers with disjoint/repeated private roots, duplicate paths, no imports/explicit-only, zero types, class-method legacy lookup, wrong Program slice/map/caller with equal text/cardinality, partial DFS failure, contains/signature work exhaustion, successful consumer followed by another caller, early consumer error, overlap/reentrancy refusal confined to the new API, and unwind/drop cleanup. Observe complete first/repeated acquisition and final Drop: absolute cumulative reservations, allocations/reallocations, allocated/freed/highwater, retained capacity, live bytes and first sticky refusal at zero/floor/exact/one-short/overflow limits. New controls may move the budget-refusal frontier and must be reported.

Keep original custody tables/assertions intact and bind changed callers plus new modules into source audits. Update the #765 owning specification paragraph (`WORKSPACE-SEMANTIC-GRAPH-V1.md:418–430`) to distinguish fresh logical roots from reused physical backing. Full ordinary/compact graph/HIR/image/cache parity and unchanged owning application reruns remain necessary. Treat this as an implementable allocation-removal hypothesis, not measured recovery or token savings.
