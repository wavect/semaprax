# Review of the typed-failure proposal (pasted source only, not executed)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The paste has no line numbers, so I cite functions and snippets. Nothing here was compiled or run. I can't see `reserve`, `ClassifierLedger`, `budgeted_clone` or `refuse_active_required`, so I mark assumptions about them.

## Blockers

**B1. `receive` ignores the saved `first` error (error priority).**
`receive` checks `self.detached || !self.ledger.current()` and returns `refusal: Setup::Detached`. A `first: Option<Setup>` saved earlier, such as a `Budget{..}` from `reserve`, is never consulted. With "firstsavedE wins", a typed `One(diag)` or `Many` arriving after a saved `Budget` has no defined result, and the foreign-ledger path would overwrite the saved error with `Detached`. `receive` must consult `first` before the `detached` test and before the `match`. Otherwise a `Received` result can silently drop a recorded setup failure.

**B2. The "captured maximum" is not captured anywhere.**
`acquire` calls `super::super::active_builder_limit()` only inside the failure arms. The `Receiver` struct has no `maximum` field, and the proposal's added field is only `first`. When a typed helper's `reserve` later fails, or when `physical()` runs, `active_builder_limit()` reads whatever authority is active then. Under a foreign owner, that is the foreign limit. The maximum must be read at `acquire`, after the successful reserve, and stored in the `Receiver`.

**B3. `physical()` touches the active ledger without authentication.**
Today `physical()` unconditionally calls `refuse_active_required(usize::MAX)`, so under a foreign active ledger it makes that other owner's ledger sticky. The typed copy must check `ledger.current()` first. If the check fails, it saves `Detached` and must not call `refuse_*`. If it passes, it refuses and then saves `Budget{captured}`, never `Allocation`. The same gate must cover all three physical exits in `append`: the checked-mul overflow, the `Layout::array` error, and the `capacity != target` check. Any one of them left on the old path would poison a foreign ledger.

**B4. Copying `reserve(size_of::<Result<…, Failure>>())` verbatim pays for the wrong carrier.**
`make`, `append` and `build` charge `Result<FieldInitializer, Failure>`, `Result<(), Failure>` and `Result<Vec<_>, Failure>`. The typed copies return `TypedFailure`, which has a different layout. `Failure` is `One(Diagnostic) | Many(Vec)`, while `TypedFailure` adds `Setup`, so the sizes may differ. The charges must name `TypedFailure` and the receiver and borrowed-frame types actually used. A `size_of` test per helper should compare the charged type with the returned type. Otherwise the layouts are paid "by name" only, which is the concern you raised.

**B5. `receive` on `&mut` or reused backing can reallocate unpaid.**
`receive` pushes into a `Vec` allocated with exactly capacity 1, with no `len == 0` guard. `receive` takes `self` by value, so it is affine today. The new helpers take `&mut Receiver`. If any typed path ever pushes into `report` through that reference, a second push reallocates without a charge. Keep `report` private to `receive`, and add `debug_assert!(self.report.is_empty())` or make it an explicit setup error.

**B6. The `Many` path drops prepaid backing and the held ledger.**
`Failure::Many(report) => CoreFailure::Legacy(report)` discards `self.report` (capacity 1, already paid) and `self.ledger`. The proposal's "Many moves backing" is not what the code does. It moves the helper's `Vec` and drops the receiver's. Whether dropping `ClassifierLedger` has side effects (release, sticky reset, TLS) is unknown. The `Many` arm also loses the "sealing held first receipt before child restore" property that `Received` keeps through `ClassifierLedgerDebug`. This is a blocker unless `ClassifierLedger`'s `Drop` is inert.

## Likely defects (confirm against missing code)

**U1. `reserve` charges each call, so per-field cost may grow.**
`append` calls `reserve(size_of::<Result<(), Failure>>())` and `reserve(0)` once per field, and `make` does two more `reserve` calls per field. If `reserve` is cumulative, as "recursive entries each paid" suggests, total charge scales with N times the carrier size. If it is a high-water or frame check, it does not. The old and typed paths must match, and tests must pin the exact boundary with N ≥ 5 so the cap-4 to cap-8 growth is exercised.

**U2. `budgeted_clone(&field.name)` has an unknown failure channel.**
Its failure is not visible in `make`. If it returns a value after silently refusing the ledger, the failure only surfaces at a later `reserve`, which changes error priority against `default_expr(...)?`. Check this against the old helper. The `?` on `default_expr` comes after the clone, so the order name-then-value must stay.

**U3. `reserve(0)` as a stickiness probe needs the authenticated path.**
The proposed "short0" controls must go through `ledger.current()` first. Otherwise a detached receiver would pass a zero reserve.

**U4. `setup_bytes` grows.**
`Self` is measured through `size_of::<Self>()`, so the enlarged `first` field is counted automatically. The new typed carriers are not in the list. If they are added there, every `acquire` charges more, which shifts every existing budget-boundary test. If they are charged at helper entry instead, as proposed, then `setup_bytes` must stay unchanged and tests must not hardcode its sum.

**U5. The `append` over-charge is unclear.**
`reserve(layout.size())` charges the full new array, not the delta over the old one. Whether this matches the old `Result<Vec>` collection depends on how the old code charged it. A post-`try_reserve_exact` failure also leaves the paid bytes spent. The old path may have done the same, so compare.

**U6. `CoreFailure::Unreceived` embeds `Failure`, but typed flows carry `TypedFailure`.**
Define the conversion for `Setup(s)` and for `One`/`Many` combined with `first`. The proposal does not say whether `TypedFailure::Setup` maps to `CoreFailure::Setup` (dropping the `Receiver` and its `ledger`) or to `Unreceived`. This is a drop-order and sticky-state decision.

## Test gaps to require

- **Foreign ledger:** each physical exit in `append` and `reserve(0)` under a foreign active ledger. Assert that the foreign ledger is not made sticky and that `Detached` wins.
- **Sticky then second error:** a first `Budget` followed by a later `One`. The first error must win, and `receive` must return the saved setup error without pushing.
- **Old versus typed equivalence:** run the retained old `build` and the typed `build` over empty, 1, 4, 5 and 9 fields. Compare diagnostics, final ledger usage and the failure frontier of `first` before `Vec` creation.
- **`G173Allocation` guard:** forced `try_reserve_exact` failure must give `Budget`, never `Allocation`.
- **Drop controls:** drop `Receiver`, `CoreFailure::Received` and `CoreFailure::Legacy` under active, foreign and no owner. Assert ledger state afterwards, which `ClassifierLedgerDebug` cannot show.
- **Layout assertions:** `size_of` equalities between charged and returned carrier types.

## Unchanged by this proposal

- `acquire` still double-counts types in `setup_bytes`, which is only conservative.
- The `Legacy` and registry/layout/cost `Vec` producers remain uncertified, as the proposal states.
