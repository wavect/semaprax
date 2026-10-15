# OPT819 ordinary DAG teardown: source-only review

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

This is based only on the pasted source. I did not see `SkeletonWork`, `ReplayBudget` ownership, `Observation`, the memo, or `pays_allocations` callers, so several points below are conditions to verify.

## 1. Order preservation

The existing selected branch of `Drop` already pops and unwraps in the same order as the legacy branch:
- `drain_children` appends in `take_children` order.
- Pops are LIFO.
- `try_unwrap` failures drop the `Rc` at the same point.
- A node's own tail drops at the end of its `if let` body, before its children are processed.

So `Drop` needs no reordering. The missing piece is that ordinary nodes are built with `scratch: None` and never register edges. The minimal change is on the construction side.

## 2. Minimal construction design

**Single choke point.** Put everything in `ordinary_dag` (`ordinary_union`, `ordinary_extend_owned`, `ordinary_prepend` and the test helpers all funnel through it). Keep `next_ordinary_dag` as the ordinal source. Do not move ordinals into the scratch counter, because that would renumber nodes and alter the `charge` label.

**Ordering inside `ordinary_dag`:**
1. Do the existing `charge(1, ...)`, the checked ordinal and `reserve_bytes(node)` unchanged and first.
2. Then, only for a non-leaf node on a paying ledger with all children covered:
   - lazily pay for and create the scratch header with a new label;
   - register the edges with `reserve_vec(pending, registered + edges, <new label>)`;
   - update `registered_edges` only after success.
3. Then `Rc::new(ObservationDag { ordinal: Some(ordinal), kind, scratch })` and commit `next_ordinary_dag`.

No fallible step sits between registration and allocation. Use new phase strings so old refusal text is untouched.

**Scratch ownership.** The ordinary scratch needs to span every `SkeletonWork` under one replay ledger, because ordinary prefixes legitimately cross works through memo hits and borrowed `&Observations`. A per-`SkeletonWork` scratch would turn those into foreign prefixes. So:
- Hold it on the ledger (or an equivalent owner outliving works) as a separate field from `dag_scratch`, so selected semantics are untouched.
- Every non-leaf ordinary node holds a clone. That is how the backing survives until the final `Drop`.
- `pending` is empty outside `Drop`, so there is no Rc cycle at rest.

**Child classification** (by `Rc::ptr_eq` on the scratch, never by ordinal):
- **Same scratch:** covered.
- **Leaf** (`!has_children`, any scratch or none): covered, because its drain pushes nothing.
- **Anything else** (foreign scratch, other ledger, legacy non-leaf): uncoverable without a traversal that would add work or a cross-scratch coverage graph. Make this node legacy (`scratch: None`, no registration). The same rule makes every ancestor legacy, so taint propagates upward.

Tainted graphs keep the old allocating destructor and old charges exactly. Do not refuse, because that would change first refusal. The "allocation-free" claim is then conditional on no taint. Ideally prove that taint is unreachable from the ordinary entry points.

**Gating.** `reserve_vec` returns `Ok` early when `!pays_allocations()`, so `pending` would never grow. If nodes held scratch there, `Drop` would allocate on append. Attach scratch only when the ledger pays. Unobserved or detached ledgers must stay legacy.

## 3. Concrete pitfalls

1. **Layout:** `rc_backing_bytes::<ObservationDag>()` depends on `size_of`. Do not add a field or change the scratch slot's type (a two-Rc enum loses the niche), or every byte threshold shifts.
2. **Byte refusals inherently move earlier.** The new bytes come from the same pool. Work refusals are unchanged, since nothing is charged in work units. Byte-capped first refusal can occur at or before the old point, with a new label. Claim no credit, no cap change and no work gain. Do not release the scratch bytes at final `Drop`.
3. **Ordinary traversal work is probably unpriced.** The ordinary path charges 1 unit per node, versus `2*edges+2` in `charged_dag`. I couldn't confirm where, if anywhere, the ordinary route prices traversal. Do not silently add units; report it as a separate gap.
4. **Reentrancy:** the node's `Vec<Observation>` drops while `RefMut` is held. If `Observation` can own or drop an `ObservationDag` from the same scratch, `borrow_mut` panics. Verify it is plain data.
5. **Never fall back to `try_borrow_mut` plus an allocating path.** It hides violations of the "no allocation" claim.
6. **Capacity invariant:** the bound is cumulative registered edges over all reachable nodes of this scratch. An unregistered node, even a leaf later given children, breaks it. A silent `Vec` regrowth is undetectable in release builds, so only a test can catch it.
7. **Partial registration on late failure:** capacity may exceed what the created nodes need. That is harmless but retained; do not shrink it.
8. **Late-refusal drops:** error paths drop partially built `nodes`, unconsumed `members` and cloned prefixes. All of those must drop allocation-free without calling `budget_refusal`, so the prepaid refusal text stays unspent.
9. **Ordinals are not identity.** They collide across scratches. Memo hits, same-scratch checks and dedup must use `Rc::ptr_eq`.
10. **Duplicate Alt members and shared prefixes:** these are multisets. Registration counts edges per node, not per holder.
11. **Test-only paths:** `ordinary_freeze` and `ordinary_extend` also feed `ordinary_dag`. The oracle's `fail_physical` hook in `budget_refusal` will see the new refusal points.
12. **Update the `Drop` doc comment.** The legacy branch becomes tainted, unpaid or leaf-only.
13. **`reserve_vec` exactness:** `try_reserve_exact` plus the `capacity != target` check can refuse if the allocator over-allocates. This is existing behavior, but it now applies to ordinary construction too.

## 4. Required controls

- **Old oracle:** verbatim copies of the old `ObservationDag`, its old `Drop` and the old `ordinary_*` constructors, compiled in a separate test module that does not share the new `Drop`. Replay identical scripts through both.
- **Shapes:** deep chains, balanced and wide Cat/Alt, diamonds via borrowed `&Observations` prefix clones, duplicate Alt members, empty leaves, externally held `Rc`s released at varied times, a foreign-ledger prefix, a legacy non-leaf prefix (taint) and an unpaid ledger.
- **Order trace:** drop-probe `Observation` tails recording the order of unique-node destruction, which also reveals which parent edge released shared nodes last. Compare to the oracle exactly.
- **Allocation:** a counting global allocator shows zero allocations inside `Drop` for paid, untainted graphs. The old oracle must show a positive count, to prove the meter works. Tainted graphs are expected to be positive.
- **Capacity:** `pending.capacity()` is unchanged across every `Drop`, and `pending` is empty at its end.
- **Budget sweeps:** sweep work caps (first refusal identical to the oracle) and byte caps (new first refusal at or before the old one, with only new labels differing and unlimited-byte runs identical in output).
- **Late refusal:** force refusal at every new check, then drop all partial state with zero allocation and no panic. Check the sticky refusal text is not consumed by `Drop`.
- **Retention:** a `Weak` to the scratch upgrades while any node lives and fails after the last drop.

## 5. Why there are no full code sketches

I cannot prove safety without the ledger and `SkeletonWork` ownership, `pays_allocations` semantics, the `Observation` drop behavior, and whether foreign legacy prefixes can occur in paid ordinary runs. The step ordering and classification rule above are the parts I consider sound. Everything else depends on those four facts.
