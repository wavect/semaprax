# Review of the final-refusal publication seam (source only, nothing executed)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

I found no definite compile-time or logic blocker in the pasted `report` and `attempt` code. Both listed blockers below depend on code that wasn't pasted.

## Blockers (conditional on unpasted code)

**B1. Allocations inside `seal` that aren't prepaid (`report`, `seal`).**
- `Diagnostic::io("SPX-G171", self.message).with_help(self.help)` is the only step after the budget is committed.
- If `io` copies the code into a `String`, or `with_help` takes `&str` or otherwise reallocates, `seal` allocates heap that `acquire` never charged. It also breaks the "no allocation on publication" claim.
- `backing` charges only `size_of::<Diagnostic>()` plus the two string capacities.
- This is safe only if both calls move their `String` arguments and the code is `&'static str` or `Cow`. Please confirm the signatures.

**B2. Two-pass `Display` of `BorrowedBuilderHelp` (`acquire`).**
- `base_len` is counted once via `Count`, then written once via `Fixed`.
- If the TLS content it reads can change between the two passes, the second write can fail with `Setup::Capacity`. It can also under-fill, which breaks the `suffix_max` bound.
- This is harmless only if the content is immutable for the duration of `acquire`. I can't verify that from the paste.

## Confirmed non-bugs

- **Typing:**
  - The `&mut Count` → `&mut dyn Write` coercion works through the `impl FnOnce(&mut dyn Write)` bound.
  - `write_char` is in scope via `fmt::Write`.
  - The inline `{maximum}` capture in `write!` is fine.
  - `try_fold(0usize, usize::checked_add)` is fine.
  - The `size_of::<Fixed<'_>>()` turbofish is fine.
- **Partial moves:** `seal`, `into_report` and `into_parts` all move fields out of types with no `Drop` impl. Nothing in the paste shows a `Drop` on `Owner`, `Published`, `Unpublished` or `Attempt`. If one is added, these become compile errors.
- **Borrows:** The `self.report.push(... self.message ... self.help)` call uses disjoint fields and a two-phase borrow, so it is fine. The immediately-invoked `FnMut` closure in `seal` is legal.
- **Capacity bound:** The `help_capacity` formula matches `seal` exactly. The separator is 1 byte, and `suffix_max` covers the `usize::MAX` fields, the longest stage and the `None` text. `Fixed::write_str` cannot underflow because `len <= capacity`.
- **Authority and ordering:** `seal` consumes `Owner` and returns it on failure. It runs before the `with_limit` restore. `into_parts` runs after the restore. The ledger `Rc` is held only until `into_report` drops it.

## Concrete defects and weak points

1. **`seal` is not idempotent after a Capacity failure (`report`, `seal`).**
   - The `' '` is appended to `help` before `evidence` can fail.
   - The `Err(Unpublished{Capacity})` path returns an owner with `detached == false` and a mutated `help`. `Unpublished.owner` is `pub(super)`, so a re-seal would double the separator.
   - Today `attempt` drops it immediately, so this is latent. Either make the owner non-reusable or write the separator and evidence atomically.

2. **The secondary failure reason is discarded (`attempt`, `into_parts`).**
   - `let _reason = unpublished.refusal;` throws away `Detached` and `Capacity`. A drifted `stages::longest()` would therefore silently fall back to legacy text.
   - That matches "secondary never replaces primary", but it leaves nothing for a test or debug assertion to observe. Consider a `debug_assert!` or a test hook.

3. **`attempt::reserve` can mislabel no-owner as `Budget` (`attempt`, `reserve`).**
   - It has no `active_remaining().is_none()` pre-check like `acquire`'s. If `reserve_active_required` returns false when nothing is active, the failure is reported as `Budget`.
   - It also looks unused in the paste, which is a dead-code warning if warnings are denied. You said NoOwner residuals stay open, so I'm noting it only.

4. **`maximum` versus `active_builder_limit()` (`report`, `budget()`).**
   - The message and help text use the `maximum` argument. The `Setup::Budget` payload uses `active_builder_limit()`. If these can differ, the typed refusal and the published text disagree.

## Uncertainties on parity and accounting

- **Prepay shifts budget.**
  - The control and backing reservations succeed before `original_core` runs and are never refunded. Inputs that previously fit near the limit can now refuse.
  - The `requested`/`remaining`/`floor` values in the first receipt also differ from the legacy values. That is consistent with "no refund", but legacy-number expectations in tests will change.
- **A failed `acquire` reservation can be the ledger's first sticky refusal.**
  - `run` then returns `CoreFailure::Setup` without ever running the core. If the outer `with_limit_usage_refusal` or legacy fallback reads the sticky receipt, it may attribute that refusal to the "resolved-core phase". I can't tell from the paste whether it does.
- **Excess handling can swap one setup error for another.** After over-allocation, `reserve(excess)?` can return `Budget` instead of `Capacity`. Both are `Setup`, so the primary is unaffected.
- **Success with a sticky refusal.** `run` seals whenever `active_refused()` is true, even if `operation()` returned `Ok`. The report is built and then possibly published alongside an `Ok`. Confirm the caller ignores the publication in that case.
- **Stack sizes as a proxy.** `control_bytes` charges `size_of` values, not heap. That is a conservative choice rather than an error, and double-counting `Result<Published, Unpublished>` is harmless.

## Tests

`mod tests;` and `mod stages;` were not pasted, so I can't confirm the "all 39 labels" claim. If `longest()` is a hand-maintained list, add a check against an exhaustive stage enumeration, not a copy of the list.

Tests I'd want:
- **Exact fit:** capacity exactly equals the final length for the longest stage and for `None`.
- **Detached:** `Detached` is returned and `into_parts` yields `None`.
- **No heap in `seal`:** a counting allocator shows zero allocations during `seal`. This closes B1.
- **Primary preserved:** an `Unpublished` outcome leaves `result` bit-identical.
- **Setup failure:** `acquire` failing yields `Setup` without invoking `operation` (counter).
