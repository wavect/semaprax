# Review of `validate_exits_retained` status-directory reuse (source only, nothing compiled or run)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The pasted source has no line numbers, so I cite quoted snippets. Nothing here is proven to fail. The items below are what the source supports.

## Blockers (given your stated goal)

**B1. First-failure order changes in the caller.**
`validate_status_sources_retained(...)?` now runs before `validate_regions` and `validate_entry`. It does the full status/expression/lane/producer/callee validation and prepays two full `StatusSources` results.
- A function with both a region/entry fault and a status fault now reports the status diagnostic first.
- A tight budget can now refuse in the producer, with a producer label, before regions or entry ever run.
- That goes beyond the "intentional paid admission" you describe. Either pin it with tests and document it as intended, or move the producer call to the position where the old status build first happened (inside blocks). If the old build was already at the start of blocks, the reorder is only relative to regions/entry. Confirm that against the old caller, which I can't see.

**B2. Tree (`ids == None`) paid path debits more than the old receiver.**
The sequence is `reserve_bytes(BORROW_BYTES)`, then `charge(BORROW_UNITS)`, then `let Some(statuses) = ... else { return old validate_exits }`.
- The old receiver then repeats its own zero reserve/charge and `Directory::new`.
- No borrow ever happens on this path, so the borrow controls are paid for nothing.
- A budget that succeeded exactly before can now refuse here, with the label "retained exit status borrow controls".
- This is acceptable only if you explicitly accept it. Otherwise test `ids.as_ref().is_none()` before the borrow debit. That keeps your ordering rule, "zero/current/sticky before ids borrow", because the zero checks have already run.

## Likely defects and compile risks

**C1. Path resolution in the new child module.**
Inside the child, `super` means `retained`, not the old `remaining` module. These items must resolve through `retained`'s own names or glob imports:
- `super::CleanupRegionId`
- `super::validate_owned_edge_paid`
- `super::require_status_paid`
- the bare `block_exists`, `expected_exit_regions`, `u32_index`, `validate_result_source`, `replay_error`, `UNRESOLVED_NORMAL_END`, and `Leaves`

If `retained` lacks any of these, the build fails. A glob chain only passes through names visible at each hop, and `pub(super)` on `selected` and the finalizer helper widens visibility only to `receiving`.

**C2. `BORROW_BYTES` slightly under-covers its own comment.**
- `validated.ids.as_ref()` takes `&Option<Directory<_>>`. That argument is a pointer that is not counted. `Option<&Directory>` is counted once for the return and `&Directory` once for the pattern binding.
- The two `&StatusSources` terms cover the caller temporary and the parameter.
- `BORROW_UNITS = 5` matches the comment's five items. The comment says "optional-return assignment, pattern receiver and branch", but the `as_ref` call itself is not listed.
- Whether a one-pointer undercount matters depends on your accounting model. If the byte accounting is meant to be exact against a tight-budget test, add a term or document why it's excluded.

**C3. Ledger check ordering for non-paying budgets.**
`validated.ledger.current()` runs before the `pays_allocations()` fallback.
- On a non-paying budget it can return `budget_refusal(...)` even though no allocation accounting applies.
- `current()` also "poisons the held ledger" through a `&StatusSources`. That needs interior mutability, so check that this is `Cell`/`RefCell`-based and that re-entrant or nested calls can't double-poison.
- Confirm that refusing on a non-paying budget is intended. The old code never refused there.

## Uncertainties (can't resolve from pasted source)

- **U1. Pointer comparison.** `std::ptr::eq(budget, validated.budget)` passes `&mut ReplayBudget` where `*const` is expected. This should coerce through an implicit reborrow and leave `budget` usable. It is comparison-only, so there is no aliasing issue provided `validated.budget` is never dereferenced. Confirm that nowhere in `StatusSources` or the ledger does.
- **U2. Lifetime coupling.** `function: &ResolvedFunction` and `StatusSources<'hir>` are not tied by type. Function identity relies solely on the runtime `ptr::eq`. That is sound if the token is only constructible by the producer, but a `'hir` that outlives the function would not be a type error.
- **U3. Token held across `validate_regions` and `validate_entry`.** Neither takes the token. If either can change the ledger state (foreign ACTIVE, then restore), `current()` is only re-checked in blocks and exits. Check whether a transient mid-sequence change is caught.
- **U4. Pre-existing panicking indexes.**
  - `plan.regions[child.0 as usize]` indexes by an ID taken from `item.id`.
  - `plan.exits[region.normal_scope_end.0 as usize]` is unchecked when lexical control is `None`.
  - These are safe only if regions and exits were validated earlier. Regions are validated just before, but confirm contiguity and range checks.
- **U5. Equivalence of the old and new status result.**
  - The old constructor rejected duplicate status IDs through `statuses.insert`.
  - The new path relies on the producer rejecting them before it builds `ids`.
  - Confirm the producer's insert path errors on duplicates the same way. Also confirm `Directory::insert` budgets don't differ from the old per-insert cost, since that cost is now prepaid as "2 full results".

## Test gaps (the `tests` module isn't shown)

Each of these should be a distinct test with the exact label asserted:
1. Foreign function and foreign budget are refused before any debit. Compare the budget snapshot before and after.
2. Stale or foreign ACTIVE ledger: poisoning is visible afterwards, and the original ledger is poisoned.
3. Unobserved budget matches only the inactive ledger.
4. Zero reserve, zero charge, and sticky-failure ordering come before any `ids` borrow.
5. Exact tight-budget boundaries: bytes at `BORROW_BYTES - 1` and units at `BORROW_UNITS - 1` fail with the borrow labels. The exact amounts succeed.
6. The Tree/`None` path's total debit versus the old receiver (B2).
7. Caller reorder (B1): a function with both a region fault and a status fault.
8. Drop-pending behavior of the token and reserved bytes after an early error return.
9. `ReturnFailure` with an unknown or duplicate status, and with a non-unique selected status.

## Not claimed

Not verified: compilation, runtime behavior, and whole-owner17 recovery. Equivalence of unseen helpers (`require_status_paid`, `validate_owned_edge_paid`, `unique_status`) is assumed from your description.
