# Review of #816 `StatusCases` / certified receiver (source only, nothing executed)

## Blockers

**B1. `Attempt::complete` does not authenticate or check the poison flag (lines in `impl Attempt`).**
It charges whatever `function` and `budget` it is handed. It never checks `proof.failed`, pointer identity, or the ledger.
- `begin(&self)` can be called repeatedly, so attempts A and B can coexist. If B is dropped incomplete, or a `member` rejection poisons the proof, A can still call `complete` and return `Ok`. A certificate is then minted from a proof already marked failed. This contradicts the comment "every early error/unwind poisons the held proof".
- `complete` accepts a foreign `function` or `budget`. It charges the foreign payer and sets `complete = true`. This breaks the "do not consult/debit a foreign payer" rule that `authenticate` enforces. Foreign function and budget cases exercised only through `begin` and `member` would miss this.
- Suggested fix: have `complete` re-run the `authenticate` checks, or at least `failed` plus `ptr::eq` on function and budget, before setting `complete`.

**B2. The proof is not affine, despite the module doc.**
`begin(&'proof self)` takes a shared borrow. A successful `complete` leaves `StatusCases` fully valid and re-enterable. Nothing consumes or marks it, because `complete` lives only on the per-attempt flag. One proof can therefore mint any number of `StatusSources`, each paying only the small begin and complete costs. If single use is intended, this is a defect. If reuse is intended, the "affine" wording and the comment at the top of the file are wrong.

## Uncertainties (cannot confirm from the excerpt)

1. **Pointer-identity authentication (`budget: *const ReplayBudget`).** `StatusCases<'hir>` carries no lifetime for the budget. Comparing `ptr::eq(self.budget, budget)` is address-only. It has ABA exposure if a budget is dropped or moved and another occupies the same address while the proof lives. In the stated caller shape the proof is a local and the budget is the `&mut` parameter, so this is probably fine, but the type system does not enforce it. A moved budget would cause a false rejection.

2. **Ordinal alignment in the receiver.** `ordinal += 1` and its charge sit at the loop tail. The excerpt shows the changed arm without its enclosing `match`, so I cannot see the other arms. Any `continue` or early path in an unseen arm would desynchronize `ordinal` from `rows`. That would make `member` reject valid rows, or accept a shifted row if two arms skipped symmetrically. The pasted loop is also not syntactically complete, because the orphaned arm has no `match`. I cannot confirm exhaustiveness or that non-arithmetic arms still run their old checks.

3. **Facts not bound to the proof.** `validate` walks via `program`. `validate_status_sources_certified` takes an independent `facts: &Facts<'_>`. Nothing in `StatusCases` binds that `facts` to the same program or function. The arithmetic arm now relies on the old walk instead of re-checking cases. Binding rests on the caller passing matching values.

4. **Removed arithmetic check.** The arm drops the old `Directory` nonempty and unique check. Equivalence relies on the old walk's fixed per-op lists and exact length check, which you state are unchanged. I cannot see the old walk, so I cannot verify that its row-to-source comparison covers every field the old receiver covered. For example, `normalized_cases` is now bound and ignored. `member` itself proves only that `source` is the row at `rows[ordinal]` and nothing about content.

5. **Zero-request authentication.** `reserve_bytes(_, 0, _)` and `charge(_, 0, _)` are asserted to surface owner sticky errors. That is a `ReplayBudget` property I cannot see. If either short-circuits on 0, `authenticate` silently loses that check.

6. **Accounting shape.**
   - `authenticate` reserves `Result<(),Diagnostic>` bytes. `member` then reserves `Option<&StatusSource>` plus another `Result<(),Diagnostic>`. This is consistent with "no credit", provided `reserve_bytes` is cumulative and never released. If it is a live-peak model, these are over-counted relative to the other receivers.
   - `complete` charges 1 work but reserves no bytes. The receiver reserves `CONTROL_BYTES` just before it, so this is probably intended.
   - `validate`'s reserve is `2*size_of::<Self>() + size_of::<Result<Self,_>>()`. That matches the comment (initializer, returned `Result`, held token), but I cannot check it against how other sites derive "exact" sizes.

7. **Toolchain.** `Option::is_none_or` needs Rust 1.82 or later. Check the MSRV. `use std::mem::size_of` is redundant on 1.80 or later, which could trigger a lint depending on the crate's lint level.

8. **H006 string.** The code uses `"SPX-H006"`. You describe the constructor as `H006`. Confirm this matches the other H006 sites and that `Diagnostic::io` takes `&str` plus `String` exactly as written. Otherwise the "zero-alloc full six-field" claim fails on a type or content mismatch.

9. **Borrow check in the receiver.** `StatusInsertionOwner::new(function, &ids, budget)` is held across `ids.insert_certified(..., &prefix_owner)`. This compiles only if the owner is not a live shared borrow of `ids`. It is inherited from the old receiver, so likely fine.

## Checked and consistent
- `&mut ReplayBudget` coerces to `*const` at the field initializer and in `ptr::eq`. `Attempt<'_, '_>` in `size_of` is legal.
- `begin`, `authenticate` and `member` propagate the first actual budget error unchanged and poison on it.
- `reject` sets `failed` before returning. A later `begin` yields H006 with no payer consulted.
- `Attempt::drop` poisons on every non-`complete` exit, including unwind. `complete` leaves the flag false if its own charge fails.
- `rows: &Vec` to `&[StatusSource]` coercion at the field site is fine. The `Cell` and raw pointer make the proof `!Send`/`!Sync`, which is acceptable if single-threaded.

## Tests
Only `mod tests;` is declared, so I cannot review the test bodies. Beyond your list, I recommend adding:
- two live attempts with one poisoned;
- `complete` with a foreign function or budget;
- a second successful `begin` and `complete` on one proof;
- an ordinal desync case.
