# Diagnosis: `fact_errors.rs:97` panic (source-only, nothing compiled or measured)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Verdict

Yes, the `cfg(test)` `Payer::Oracle(OracleLease)` variant is the likely cause. It is never constructed on this path. It changes the layout of `Payer`, and therefore `size_of::<FactCapacity>()`. The test's layout twin has no such variant, so its `cursor` term is too small.

## Which case panics

`success` is `true` only for the `(full, true)` row. `assert!(!success)` in the `Err` arm can therefore fire only when the actual charge at `limit == full` was refused. So the real cost exceeds the formula's `full`, and the formula undercounts.

An initial report of "full-1 success" cannot be reconciled with this source. At `full-1` the test expects `Err`, and a success there would trip `assert!(success)` in the `Ok` arm instead. Check which assertion line the frozen panic actually reports.

## Formula vs actual

The route is `expression_facts::paid` → `with_policy::<Selected>` → `Payer::Selected(physical)` → `FactCapacity::with_payer`. The actual charges are:

| Charge | Actual | Test formula |
|---|---|---|
| Cursor | `size_of::<FactCapacity>() + size_of::<[FactFrame;514]>()` | `size_of::<CapacityLayout>() + 514 * size_of::<Frame>()` |
| Per-expression tree node | `btree_node_upper::<ExpressionId, Option<CallFact>>()` in `expression()` | same call, times 2 |
| Types | `types()` is never called (no `Invoke` or `Call`) | not counted |

`tiny()` has a `Block` and an `Int`, so there are two node charges, matching `2 * node`. The only structural mismatch I can find is `Payer` versus `PayerLayout`.

- **Twin:** `Selected(&mut _)` plus a dataless `ActiveBuilder`. The null niche of the reference should let this fit in 8 bytes.
- **Actual in test builds:** `Selected(&mut _)`, `ActiveBuilder`, and `Oracle(OracleLease)`. `OracleLease` is a `u64` epoch plus a `PhantomData`, with no niche. Two data-carrying variants whose payloads overlap the niche field cannot use niche encoding, so rustc should add an explicit tag. I expect 16 bytes, but repr(Rust) layout is unspecified and I have not measured it.
- **Effect:** `FactCapacity` is expected to be about 8 bytes larger than `CapacityLayout`. The `514 * Frame` term is unaffected. The `(base + cursor - 1)` and `(full - 1)` rows still refuse as expected, which is why only the `full` row breaks.

The same size difference would also fail the `selected.usage().1 == full` assertion in the `Ok` arm if the charge had succeeded.

## Exact repair (test file only)

Keep the numeric assertions, caps, observer, old receiver and production code unchanged. Extend the layout twin with a literal third variant that does not import the real type:

```rust
struct LeaseLayout { _epoch: u64, _thread: std::marker::PhantomData<*const ()> }
#[allow(dead_code, reason = "independent physical layout twin")]
enum PayerLayout<'a> {
    Selected(&'a mut PhysicalReservation),
    ActiveBuilder,
    Oracle(LeaseLayout),
}
```

Comment that the twin matches the test build, because the variant is `cfg(test)` and the tests only run in that build. Note also that production and test builds charge different cursor sizes, which is worth a separate flag.

## Missing guards

1. **Size-equality guard against drift.** Add a `#[cfg(test)]` assertion next to `FactCapacity` in `capacity.rs`, or a `pub(in crate::cleanup_plan::replay)` test accessor. It should check `size_of::<FactCapacity<'static,'static>>()` against the twin's size, and `size_of::<Payer>()` against `PayerLayout`. The test module probably cannot see the `pub(super)` type directly. Without this guard, any new variant or field breaks the formula silently.
2. **Frame equality.** Check that `FactFrame<'_>` is the same type as `Option<(ReplayChildCursor<'_>, bool)>`, by type-identity coercion or a size and align check. I could not see `FactFrame`'s definition, and the test's `Frame` alias is an assumption.
3. **Pin the node count.** Assert the census is exactly two expressions, so `2 * node` is tied to `facts.len() == 2`. The `Ok` arm already checks this only on success.
4. **Boundary delta.** Assert that `full - 1` fails on the last node charge, not the cursor, for example by comparing the refusal text or the ledger's remaining bytes. This distinguishes an undercount from an off-by-one.
