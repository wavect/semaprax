# claude result (paid-classifier-receiving-f8)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Verdict: not established.** Nothing in the supplied source shows that `work(0, …)` can observe `overflowed`.

**1. `identity()` fix**
- **The gap is real.** `identity()` returns `Ok(value)` whenever `storage.first` is `None`. A strict logical decline never sets `first`. `strict()` swallows the failure, clears `bytes`, and returns `Ok(())`. `inventory_text` then returns `Some("")`, so `identity()` publishes success. That matches a short case returning `Ok` where `short.1` expects `Err`.
- **The proposed `work(0)` may do nothing.**
  - `Storage::work` only forwards to `payer.inventory_work(0, phase)`.
  - In the shown `charge`, a zero-unit charge can only fail through `refusal_spent`, `builder_refused()`, `charge_skeleton`, or `charge_oracle_work`.
  - `builder_refused()` is truncated, and `Payer::inventory_work` is not shown.
  - Nothing visible reads the bounded-output `Budget.overflowed`. `ClassifierLedger::current()` only writes it, and `checked_sub(0)` cannot fail.
- **Confirm before relying on it.**
  - Unless `inventory_work` (or `builder_refused` through `record_refusal`) maps overflowed or refused state to `Err`, the fix leaves `Ok(value)` unchanged and line 71 keeps failing. Check that first.
  - If it doesn't, the reliable fix is a small read-only accessor in `bounded_output`, such as `active()`'s `overflowed.get()`. `identity()` would call it after `key::identity` and return `Err(construction_refusal(...))`.
  - That adds no caps or quota and leaves the old formatter untouched.
- **Use `let _ = storage.work(0, …)`, not `?`.** The function returns `Result`, and `work` returns `Option`. `work` also early-returns `None` when `first` is already set, so the existing `match` on `first` still decides the outcome.
- **The retry assertion needs a check at entry too.** In `observe`, a failed query is retried and `allocated` must stay unchanged. If the overflowed check only runs at the end, the retry can re-run the formatter and allocate before it fails. Check sticky overflowed at entry as well, before `key::identity`.

**2. H006 and `prepare_builder` in the test setup**
- `inventory_text::current` returns H006 when `!pays_allocations()`, meaning a `LegacyUnobserved` payer.
- `prepare_builder` returns `Ok(())` immediately when `!pays_builder()`, so it is a no-op for that payer and cannot cure H006. The raw test must construct its budget with `ActiveBuilder`, `Oracle`, or `SelectedReservation`.
- In `constructors.rs`, `observe` already calls `budget.charge(...)`, which calls `prepare_builder` first. An extra call there is redundant. The skipped-preparation problem applies to the raw test that goes through `reserve_program_skeleton_work`, not to this file.
- If you add `prepare_builder` to the raw test, put it inside the measured `with_limit_usage` closure. Its 512+ID-length reservation is then counted identically in the ample, exact, and short runs, and the `used == paid` assertion stays valid.

**3. Oracle fast receiver (scope-open 0 → work 0)**
- The new paid inventory charges per visit, so it will always be refused under a zero-work grant. Don't compare it against the old result there.
- Keep the old zero-work body and test unchanged. Add a separate control for the new paid constructors that covers finite work, exact, short, and zero. Zero must expect `Err`.
- Never route the new classifier through the zero-work fast path. That would be uncounted dispatch.
- The `work(0)` authentication unit costs nothing, so it doesn't disturb zero-work receivers.

**Byte-exact test**
- The `.before` comparison covers `hir.rs`, `paid.rs`, `key.rs`, `substitute.rs`, and `storage.rs`. A fix confined to `inventory_constructor.rs` (and `bounded_output`, if you add the accessor) doesn't touch them.
- Don't modify `paid.rs`, because its tail strip is exact.
