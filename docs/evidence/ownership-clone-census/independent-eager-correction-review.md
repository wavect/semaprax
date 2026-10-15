# independent eager correction review (ownership-clone-census)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Verdict:** one definite accounting regression, plus two compile-risk items I couldn't rule out from the pasted text. I did not see anything wrong with the census cursor accounting itself.

## Definite error: `Payer::new()` charges on the fast path

`borrowed_ownership_copy` now evaluates `Payer::new()?` in the `else` arm. That runs before `type_flags_paid` reaches its own fast path, which covers primitives and cached facts. So every primitive or cache-hit query now pays `size_of::<Payer>()`, which is 1 byte, where before it paid `bytes(0)`.

This breaks the existing, unchanged test `primitive_copy_queries_remove_actual_layout_acquisition_and_authenticate_sticky_zero`:
- It runs `expected_ownership(&I64, Own)` and `is_owned_resource` under `with_limit_usage_refusal(0, ..)`.
- It asserts `!refused && paid == 0 && first.is_none()`.
- With the patch, the first `Payer::new()` refuses at limit 0, so that assertion fails.
- This also contradicts the `paid.rs` comment that the fast path needs no Storage carrier, no allocation and no allowance.

The inline payer is already funded again by `Storage<P>` in `type_flags_paid_construct` when it moves in, because `Storage { payer, first }` contains it. The eager charge adds a double count and charges queries that never construct anything.

**Recommended fix:**
- Make `new()` infallible: `pub(super) fn new() -> Self`.
- Pay `size_of::<Self>()` together with the cursor inside the `!self.clone_cursor_paid` block of `clone_type`, so it is still once per payer and before census.
- Update the hook to `Payer::new()` with no `?`.
- Update the new tests to match:
  - `setup` stays the same.
  - The limit-0 case then refuses at the first clone rather than at `new()`.
  - The `match Payer::new()` and `.unwrap()` wrappers go away.

If the parent insists on eager funding, the old primitive test has to change, which contradicts "old tests unchanged".

## Compile risks to check (can't verify from what was pasted)

1. **Other `Payer` call sites.** `Payer` changed from a unit struct to a struct with a field, so any bare `ownership_flags::Payer` used as a value fails to compile. The patch fixes only the hook at `nominal_facts.rs:223`. Please grep `ownership_flags::Payer` and `Payer)` in:
   - `validation.rs`
   - `oracle_entries.rs`
   - `call_parameters.rs`
   - `before.rs`
   - the `declaration_index` tests
2. **Metrics equality.** The new tests use `assert_eq!(before, current_metrics())` and `assert_eq!(same, wide)`. The existing test compares `(allocations, allocated)` tuples instead, which suggests the metrics type may lack `PartialEq` or `Debug`, or that some fields legitimately differ. If so, compare fields explicitly.

## Checked and fine

- **Provenance JSON:** the updated hook-3 `from` string matches the new source text. `to` is unchanged and the count is still 1.
- **Census cursor:** the `[Option<(&ResolvedType, usize)>; 514]` size matches `frames` in `clone_payload_bytes`. Depth 514 passes, since the push check only fires at `len == 514` when a child exists, and depth 515 returns `None`, so `pay(None)` raises the "layout overflowed" failure.
- **Per-clone charge:** `authenticate()` runs on every clone, and the cursor is charged once and only after a successful pay. A failed pay leaves the flag false, but sticky refusal makes any retry fail at `authenticate`.
- **Payload arithmetic:** the 5×`size_of::<ResolvedType>()` payload for the test type matches both the census and the physical allocations.
- **Test mechanics:** the closure and borrow structure, the `?` inside the closure, and the glob/shadowed `size_of` and `ReplayClonePayload` imports are fine.
