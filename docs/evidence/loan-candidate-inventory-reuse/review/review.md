# review (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I reviewed only the pasted text and ran nothing. The paste has no line numbers, so I cite by function or statement. The 64/60 normalizer overrun can't be attributed to any inner operation from this source, and I don't try to.

## Blocker-level, concrete from the source

1. **`inspect` adds work and a new error before any old-equivalent work.**
   - The membership loop calls `charge(work)` once per scanned function, before discovery.
   - The old funded candidate had no such charge, and it returned `bool`, not `SPX-H006`.
   - If the "old work receipt retained" is asserted on the funded-candidate path, this extra work breaks it.
   - If any caller passes a function that is not in `program.functions` or `function_instances`, the result changes from a bool to a hard error. Examples would be lifted, nested, template or synthetic functions. Nothing pasted shows callers are limited to those two lists.
   - Scanning per candidate is O(functions), so the whole pass is quadratic in work units.

2. **`validate` pays before it compares identity, and it pays the wrong account.**
   - It reserves on `self.payer`, then compares against the caller's `payer`.
   - With a foreign or stale caller `payer`, the charge, and any sticky `first` refusal, lands on the inventory's account, not the caller's.
   - A stale or detached `self.payer` surfaces as `storage::marker()` (a budget refusal), never as `SPX-H006`. So "foreign/stale" classification depends on which side is active. The comment says "same live account", but only the inventory's side is authenticated.
   - Check that tests pin which diagnostic each of these cases produces.

## Accounting uncertainties

3. **Overlapping reservations.**
   - `RESULT_BYTES` already includes `size_of::<Option<Inventory>>()`.
   - Each true return then reserves `size_of::<Inventory>()` again, so true returns pay more than the `Ok(None)` return.
   - `validate` runs twice, once in `build` and once in `into_parts`, and each call prepays a `Result<(), Diagnostic>`.
   - `into_parts` also pays `Result<Parts,_> + Parts` for what is only a move.
   - None of this is wrong on its face if the boundary was derived deliberately. It does mean the exact and short boundaries differ per path, so the five true returns need five separate exact and short-by-one tests. Five copy-pasted sites will drift otherwise.

4. **Backing-store retention.**
   - The comment says the paid spill is "retained", but `discovery` is a local `Cursor` dropped on return. `Inventory` holds no spill.
   - Since there is no refund, the charge persists, but the memory is gone. That is conservative, but the comment is misleading.
   - The maps now live across `build_cfg`, so peak memory overlaps the CFG. The old path dropped the first maps before the CFG was built. Confirm the reservation sizing covers that overlap.

5. **Work totals.** The new producer traverses twice: a full discovery pass, then `reset` and a replay (not shown). The old second loop's per-expression `charge(work)` is gone from the plan. Net units are not visibly equal to the old path, and "independently derived" boundaries need exact pins.

## Ordering and last-wins

6. The first loop matches the old second loop statement for statement. Parameters go in first, then expressions. Per expression: aliases, match patterns, then block/vec let bindings. Duplicate keys therefore resolve the same way, last-wins, in both loops.
   - The only divergence risk is the source of expressions: `discovery` replaces `&cfg.expressions`.
   - You state both are requires→body→ensures preorder, but the paste doesn't show that `cfg.expressions` contains no synthetic or omitted nodes.
   - Duplicate IDs are moot because the CFG rejects them before `into_parts`. But `validate` runs before `build_cfg`, so a validate failure now takes precedence over a CFG duplicate-ID error. The old order was CFG first.

## Typing

I saw no definite compile errors.

7. **Likely fine but unverified:**
   - `work.allocation` must be `Option<&CfgReservation>` and `Copy` for `payer.expect(..)` and `is_some_and`. The old diff's `Map::new(payer.expect(..))` suggests it is.
   - `program.functions.iter().chain(...map(|i| &i.function))` needs `functions: Vec<ResolvedFunction>` and `function` stored by value, not boxed or `Rc`. Otherwise the iterator item types differ.
   - The private `Parts` alias in a `pub(in crate::loan_plan)` signature is fine, because aliases expand.
   - The `pub(super)` fields are only constructible from `paid_candidate` or its descendants. That is fine if the five return sites live there.

8. **`expect("private funded entry")` after `validate`.** It is safe only because `validate` returns `Err` on `None`. That is fragile coupling and worth a comment.

## Tests

No tests were pasted. At minimum, I would want:
- a foreign program, a foreign function, a foreign reservation, and a stale/detached owner whose replacement reservation has a different address
- the same budget but a different `CfgReservation` object
- exact and short-by-one at each of the five true returns and at the `Ok(None)` return
- `inspect` called with a function outside both lists
- equality of the maps against the retained `cfg(test)` full inspect and build, including duplicate-key cases
