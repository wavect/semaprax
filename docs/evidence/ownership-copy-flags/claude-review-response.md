# Source-only review: ownership-copy paid flags hooks

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**No definite compile, privacy, or Clippy blocker is visible in the supplied source.** Several items depend on files I wasn't shown and are worth a quick grep. None are failures I can demonstrate.

## Correctness: matches the stated intent

- **Nominal lookup first:** `borrowed_ownership_copy` calls `nominal_facts.get(..)?` before any declaration fallback, so error propagation and hit precedence are unchanged. Only the owning fallback moves to `type_flags_paid`.
- **Cache and diagnostic priority:**
  - `flags_paid` keeps the empty-arg `nominal_key::get` and `compound_get` precedence, including forged entries.
  - A declined `compound_get` falls back to the owned key, which is paid before construction, and an exact miss is kept distinct from a decline.
  - `type_flags_paid_construct` returns `Err(first)` over any semantic `None`, so the first physical refusal wins.
  - `payer.bytes(0)` runs before the leaf fast path, so sticky refusal beats semantic results.
- **Payment before acquisition:**
  - `Storage<P>` is paid before it is built.
  - `grow` pays before `try_reserve_exact`.
  - `clone_type`, `boxed_type` and `node` also pay first.
  - The zero-byte `authenticate` adds no credit and no allowance.
- **Frozen originals:**
  - `before.rs` bodies match the removed lines of the diff.
  - The dispatch is `#[cfg(test)] if original_selected() { return ..._before }`, which is legal statement-position cfg.
  - The guard saves and restores the previous value, so it is nested- and unwind-safe.
  - The guard is bound as `_ownership`, not `_`, so it lives to the end of the function.
- **Deficit18:** I can't attribute it from this source. Nothing here claims or hides it.

## Verify by grep (outside the supplied files)

1. **`TypeFactsPayer` in scope for `ownership_flags.rs`:** it relies on `use super::*` from `validation.rs`, and the diff adds no import. If `validation.rs` doesn't already import it, you get E0405 on `impl TypeFactsPayer for Payer`.
2. **`ownership_flags::original_guard()` in `oracle_entries.rs` is not cfg-gated, but `original_guard` is `#[cfg(test)]`:**
   - Non-test builds break unless the `oracle_entries` module is itself `#[cfg(test)]`.
   - Also confirm `ownership_flags` resolves from `identity_directory::oracle_entries`, which needs a glob chain through `identity_directory`.
3. **Helper names and signatures:** these exist and match what is used, none of which I can see: `hir_error_static`, `scope_checkpoint::charge::failure(usize, &'static str) -> Diagnostic`, `ResolvedType::clone_payload_bytes() -> Option<usize>`, `cleanup_plan::replay_btree_node_upper::<K,V>() -> Option<usize>`, `super::cached_facts`, and `TypeFlags::from(&TypeFacts)`.
4. **`assert_eq!` on `Result<OwnershipMode, Diagnostic>` in `compare`:** this needs `Diagnostic: PartialEq + Debug`. If it lacks `PartialEq`, compare `.map_err(|e| e.to_string())` instead.
5. **Evidence files:** the `include_str!` evidence files `expected_ownership.before.txt` and `is_owned_resource.before.txt` must exist. Their text must be `fn name(`-prefixed with no visibility, or the `replace` won't line up with `before.rs` (including the `#[rustfmt::skip]` lines).
6. **Zero-allocation test:** the `allocations == 0` assertion assumes `nominal_facts.get` returns `None` for `I64` without building a key. If it allocates, the assertion fails even though the new code is correct.
7. **Leaf fast path with a poisoned `I64` cache:** `type_flags_paid` returns `self.type_flags(ty)` before consulting the cache. The poisoned-`I64` comparison only passes if `type_flags` and the old `type_facts` agree on leaf precedence. The test covers this, but it is the likeliest runtime mismatch.
8. **Generic-unknown and imported-fallback whole-copy semantics:** these live in `compute.rs`, `substitute.rs` and `admission.rs`, which weren't supplied. I can't confirm them from this source, only that the test fixture exercises those types.

## Nits (non-blocking)

- **Clippy:** `borrowed_ownership_copy` is a non-test impl added after `#[cfg(test)] mod tests;` in `nominal_facts.rs`. `items_after_test_module` should skip out-of-line modules, but moving the impl above `mod tests;` removes the doubt.
- **`Payer::grow`:** it charges `target * size_of::<T>()`, the full new capacity, rather than only the extra over the existing capacity. That is conservative, but check it matches the other payers' accounting.
- **`Payer::grow`:** the `capacity() != target` check could reject an allocator that rounds up. This is unlikely for `Vec` with `reserve_exact`.
- **Closure parameter:** `|facts| !facts && ...` now binds a `bool`, so renaming it to `copy` would read better.
- **Redundant call:** `type_flags_paid` repeats the `bytes(0)` authentication already done by `authenticate()`. That is harmless and sticky-safe.
