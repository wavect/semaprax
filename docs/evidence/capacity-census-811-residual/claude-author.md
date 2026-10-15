# OPT811 telemetry packet: patch and critique

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only review. Nothing was compiled or run, and I couldn't see `legacy.rs` or `lexical_control`.

## Critique of the proposal and of the existing code

1. **Compat max.** Don't fold in-scope notes into the existing `STANDALONE` cell. That would change `lower_capacity_high_water()` whenever it is read inside an active `Construction`, and I can't see its callers. Keep `STANDALONE` as the outside-scope compat cell and add a separate `BUILDER_MAX` for the exposed standalone maximum. Compat then stays `max(STANDALONE, OVERLAP)` by construction.
2. **`staged` keeps the last value.** It does `c.staged = bytes`. Today the value is monotone in practice, since plans are only pushed. It should still be a maximum, and `valid = false` must stay sticky.
3. **Unchecked arithmetic in `staged`.** `plans.capacity() * size_of::<CleanupPlan>()` is the seed of the checked fold, so it can still overflow. That is theoretical for a real `Vec`, but it is cheap to fix.
4. **Saturation is invisible.** `upper()` saturates, but nothing marks the record invalid. A saturated sum currently looks like a valid `usize::MAX`.
5. **Invalid observations still reach `OVERLAP`.** An observation with `valid == false` still feeds `OVERLAP`, where it undercounts. The new accessor must return `None` instead of a number once any invalid observation has been seen.
6. **Unchecked arithmetic in `conditional_backing` and `string_owner_moves`.** Both use plain `*`, `+` and `.sum()`. That panics in debug and wraps in release, which would understate. Saturate there, and let a `usize::MAX` note mark the observation invalid.
7. **Nested scopes.** Restore on drop is correct for properly nested use. Two things to know:
   - An inner invocation's components are never folded into the outer one. That matches the current semantics, and I'm leaving it alone.
   - Dropping scopes out of order would hit the `expect` in `Drop`. That is test-only and I'm not changing it.
8. **Control bug in `observe`.** `LAST` is read after the `build_plan_for_execution` loop. If that function ever opens a `Construction`, `LAST` describes the wrong invocation. Capture it right after `build_program_plans`.
9. **`Components::default()` has `valid: false`.** Every site already overrides it, so this is only a footgun and I'm not changing it.

## Patch: `capacity_observation.rs`

```rust
impl Components {
    pub fn upper(self) -> usize {
        self.builder
            .saturating_add(self.staged)
            .saturating_add(self.attached)
    }
    pub fn checked_upper(self) -> Option<usize> {
        self.builder
            .checked_add(self.staged)?
            .checked_add(self.attached)
    }
}
thread_local! {
    static STANDALONE: Cell<usize> = const { Cell::new(0) };
    static BUILDER_MAX: Cell<usize> = const { Cell::new(0) };
    static OVERLAP: Cell<usize> = const { Cell::new(0) };
    static OVERLAP_VALID: Cell<bool> = const { Cell::new(true) };
    static CURRENT: Cell<Option<Components>> = const { Cell::new(None) };
    static LAST: Cell<Components> = const { Cell::new(Components { builder: 0, staged: 0, attached: 0, valid: true }) };
}
pub(super) fn reset_lower_capacity_high_water() {
    STANDALONE.with(|c| c.set(0));
    BUILDER_MAX.with(|c| c.set(0));
    OVERLAP.with(|c| c.set(0));
    OVERLAP_VALID.with(|c| c.set(true));
    LAST.with(|c| {
        c.set(Components {
            valid: true,
            ..Components::default()
        })
    });
}
/// Unchanged compatibility value: outside-scope builder maximum or overlap sum.
pub(super) fn lower_capacity_high_water() -> usize {
    STANDALONE.with(Cell::get).max(OVERLAP.with(Cell::get))
}
/// Largest single builder census note, inside or outside a construction scope.
/// `usize::MAX` means a term saturated.
pub(super) fn standalone_builder_high_water() -> usize {
    BUILDER_MAX.with(Cell::get)
}
/// Largest per-invocation sum of component census maxima. `None` once any
/// invocation had an unmeasurable or saturated term. Not an allocator reading.
pub(super) fn invocation_overlap_component_sum_upper() -> Option<usize> {
    OVERLAP_VALID
        .with(Cell::get)
        .then(|| OVERLAP.with(Cell::get))
}
pub(super) fn note_lower_capacity_high_water(bytes: usize) {
    BUILDER_MAX.with(|water| water.set(water.get().max(bytes)));
    CURRENT.with(|cell| {
        if let Some(mut components) = cell.get() {
            components.builder = components.builder.max(bytes);
            components.valid &= bytes != usize::MAX;
            cell.set(Some(components));
        } else {
            STANDALONE.with(|water| water.set(water.get().max(bytes)));
        }
    });
}
```

`Drop for Construction`:

```rust
fn drop(&mut self) {
    let mut observed = CURRENT
        .with(|c| c.replace(self.previous))
        .expect("active construction census");
    observed.valid &= observed.checked_upper().is_some();
    LAST.with(|c| c.set(observed));
    OVERLAP.with(|c| c.set(c.get().max(observed.upper())));
    OVERLAP_VALID.with(|c| c.set(c.get() && observed.valid));
}
```

`staged`:

```rust
pub(super) fn staged(plans: &Vec<CleanupPlan>) {
    let bytes = plans
        .capacity()
        .checked_mul(std::mem::size_of::<CleanupPlan>())
        .and_then(|base| {
            plans.iter().try_fold(base, |bytes, plan| {
                bytes.checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(plan)?,
                )
            })
        });
    update(|c| match bytes {
        Some(bytes) => c.staged = c.staged.max(bytes),
        None => c.valid = false,
    });
}
```

`conditional_backing` and `string_owner_moves` (saturating, same terms):

```rust
pub(super) fn conditional_backing(state: &FlowState) -> usize {
    let variants = state
        .conditional_variants
        .capacity()
        .saturating_mul(std::mem::size_of::<ConditionalFlowVariant>());
    state.conditional_variants.iter().fold(variants, |total, variant| {
        let cases = variant.cases.capacity().saturating_mul(std::mem::size_of::<(
            DeclarationId,
            Vec<LivenessFlagId>,
        )>());
        let cases = variant.cases.iter().fold(cases, |sum, (case, flags)| {
            sum.saturating_add(case.owned_allocation_bytes()).saturating_add(
                flags
                    .capacity()
                    .saturating_mul(std::mem::size_of::<LivenessFlagId>()),
            )
        });
        total
            .saturating_add(cleanup_place_owned_capacity(&variant.root))
            .saturating_add(variant.variant.owned_allocation_bytes())
            .saturating_add(cases)
    })
}
pub(super) fn string_owner_moves(builder: &PlanBuilder<'_>) -> usize {
    builder
        .string_owner_moves
        .iter()
        .map(expression_backing)
        .fold(
            builder.string_owner_moves.len().saturating_mul(
                std::mem::size_of::<ExpressionId>()
                    + std::mem::size_of::<BTreeSet<ExpressionId>>(),
            ),
            usize::saturating_add,
        )
}
```

No change to `helper_index.rs`: the `#[cfg(test)]` calls to `attached` and `staged` already fit. `attached` stays assign-once.

## Patch: `tests.rs`

In `observe`, capture the census immediately after the build and keep the rest as is:

```rust
let plans = helper_index::build_program_plans(program).unwrap();
let components = LAST.with(Cell::get);
assert_eq!(CURRENT.with(Cell::get), None);
legacy::record_construction(&plans, components.attached);
// ...original-control loop unchanged...
// delete the later `let components = LAST.with(Cell::get);`
```

In `repeated_construction_does_not_feed_previous_aggregate_back_into_builder`, after the final `assert_eq!(lower_capacity_high_water(), upper)`, add:

```rust
assert!(standalone_builder_high_water() >= first.builder);
assert_eq!(invocation_overlap_component_sum_upper(), Some(first.upper()));
assert_eq!(
    lower_capacity_high_water(),
    standalone_builder_high_water().min(upper).max(upper)
);
```

The `==` on the overlap sum holds only if `build_plan_for_execution` opens no `Construction`. If it does, relax it to `>=`.

The last assert above is vacuous, so drop it. The real compat control is the unchanged `assert_eq!(lower_capacity_high_water(), upper)`.

New tests:

```rust
#[test]
fn standalone_builder_max_counts_inside_construction_and_compat_is_unchanged() {
    reset_lower_capacity_high_water();
    note_lower_capacity_high_water(5);
    {
        let _outer = Construction::begin();
        note_lower_capacity_high_water(11);
        attached(3);
        update(|c| c.staged = 4);
        assert_eq!(lower_capacity_high_water(), 5); // compat ignores in-scope notes until drop
        {
            let _inner = Construction::begin();
            note_lower_capacity_high_water(2);
        }
        assert_eq!(CURRENT.with(Cell::get).unwrap().builder, 11);
    }
    assert_eq!(standalone_builder_high_water(), 11);
    assert_eq!(invocation_overlap_component_sum_upper(), Some(18));
    assert_eq!(lower_capacity_high_water(), 18);
    assert_ne!(
        standalone_builder_high_water(),
        invocation_overlap_component_sum_upper().unwrap()
    );
}

#[test]
fn staged_retains_maximum_not_last() {
    reset_lower_capacity_high_water();
    let mut big: Vec<CleanupPlan> = Vec::with_capacity(4);
    big.push(CleanupPlan::unresolved());
    let small: Vec<CleanupPlan> = Vec::with_capacity(1);
    {
        let _scope = Construction::begin();
        staged(&big);
        let high = CURRENT.with(Cell::get).unwrap().staged;
        assert!(high >= 4 * std::mem::size_of::<CleanupPlan>());
        staged(&small);
        assert_eq!(CURRENT.with(Cell::get).unwrap().staged, high);
    }
    assert!(LAST.with(Cell::get).valid);
}

#[test]
fn overflow_and_invalid_terms_withdraw_the_overlap_sum_and_stay_sticky() {
    let saturated = Components { builder: usize::MAX, staged: 1, attached: 0, valid: true };
    assert_eq!(saturated.checked_upper(), None);
    assert_eq!(saturated.upper(), usize::MAX);
    reset_lower_capacity_high_water();
    {
        let _scope = Construction::begin();
        update(|c| {
            c.builder = usize::MAX;
            c.staged = 1;
        });
    }
    assert!(!LAST.with(Cell::get).valid);
    assert_eq!(invocation_overlap_component_sum_upper(), None);
    assert_eq!(lower_capacity_high_water(), usize::MAX); // compat saturates, as before
    {
        let _scope = Construction::begin();
        note_lower_capacity_high_water(1);
    }
    assert_eq!(invocation_overlap_component_sum_upper(), None); // sticky until reset
    reset_lower_capacity_high_water();
    assert_eq!(invocation_overlap_component_sum_upper(), Some(0));
    {
        let _scope = Construction::begin();
        note_lower_capacity_high_water(usize::MAX);
    }
    assert_eq!(invocation_overlap_component_sum_upper(), None); // MAX note sentinel
    assert_eq!(standalone_builder_high_water(), usize::MAX);
}
```

The existing `scope_restores_on_success_refusal_and_nested_construction_without_prior_input` stays as is. Its `99` assertion still holds because `STANDALONE` is untouched. Add `assert_eq!(standalone_builder_high_water(), 99)` after the scope block. Its refusal half should also assert `invocation_overlap_component_sum_upper() == Some(0)` after the refused call, since an empty carrier is valid.

## Open items I couldn't verify

- Whether `build_plan_for_execution`, or anything else in the builder path, opens its own `Construction`. This affects the `==` versus `>=` choice above.
- Callers of `lower_capacity_high_water()` outside tests. The `STANDALONE` split keeps them safe regardless.
- Whether `owned_allocation_bytes` on `DeclarationId` returns `usize`. The existing code adds it directly, so I assumed so. If it returns `Option`, route it through `expression_backing`-style invalidation.

Naming reminder: `invocation_overlap_component_sum_upper` is a sum of census maxima. Don't describe it as an allocator or RSS observation, per the module header.
