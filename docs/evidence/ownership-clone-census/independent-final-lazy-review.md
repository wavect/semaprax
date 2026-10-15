# independent final lazy review (ownership-clone-census)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Verdict: no definite compile, API or logic errors in the added controls.** This is a source-only read of the pasted excerpts, with nothing built or run.

**What I checked and found consistent**
- **`type_flags_paid_lazy` signature:** `P` is inferred from `make_payer`. `Payer::new` (`fn() -> Result<Payer, Diagnostic>`) and `Payer::authenticate` (`fn() -> Result<(), Diagnostic>`) both fit the `FnOnce` bounds.
- **Visibility:** `pub(super)` items in `ownership_flags` are reachable from `nominal_facts` and from `ownership_flags/tests.rs`. `nominal_facts` reaches `ownership_flags` through `use super::*`.
- **Call order:** authenticate, then the primitive gate, then `cached_facts`, then `make_payer()?`, then `type_flags_paid_construct`. This matches `type_flags_paid` except that `make_payer` runs first. The construct path charges `size_of::<Storage<P>>()` exactly as before.
- **Double authentication:** `borrowed_ownership_copy` authenticates, then the lazy entry authenticates again. Both are `pay(Some(0))`, so there is no extra charge.
- **`Payer::new`:** it charges `size_of::<Self>()` (one `bool`) before publication. The test's `setup = size_of::<Payer>() + cursor` and the deep-census `paid` assertion agree with that.
- **Lazy test, zero-limit part:** with `limit = 0`, `authenticate` pays 0 and is not refused. The unknown-nominal case then fails in `Payer::new` with `requested == size_of::<Payer>()` and `paid == 0`. That is consistent.

**Unverified from these excerpts (not definite errors)**
1. **Possible panic in the zero-limit loop.** The panic closure fires if `cached_facts` returns anything but `Some(Some(_))` for one of the four forged types. The `cache:é\0.copy` nominal is the likeliest to hit this: if the bounded census declines a non-ASCII or NUL identity, `cached_facts` yields `None` and the fallback calls `make_payer`. Confirm that `cached_facts` hits for the forged compound and nominal keys, as the original fast-path tests presumably do.
2. **Last assertion in that test:** `metrics == Metrics::default()` while the `Err(Diagnostic)` is still alive. This only holds if `hir_error_static` allocates nothing. The existing sticky test and the deep-census test suggest it doesn't, but I can't see `hir_error_static` itself.
3. **`DeclarationIndex::default()`:** the test needs `Default` on `DeclarationIndex`, and the type must be in scope in `ownership_flags/tests.rs`. Neither appears in the excerpts. The `TypeFacts` literal field set is also unchecked.
4. **Dead-code warning:** `type_flags_paid_original_receiver` is `cfg(test)` and duplicates `construct`. It may warn as unused if nothing calls it, which only matters under `-D warnings`.
