# review (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Scope:** I reviewed the pasted source only and ran nothing. I found no definite compile or logic blocker in the pasted code. The risks below are uncertainties that need confirmation outside the paste.

## Blockers

None confirmed from the paste.

## Likely-correct points

- **Ordering and duplicates:** `Keys` walks words in ascending order and bits low to high, so output is strictly ascending. `insert` returns `fresh = false` for duplicates, and `len` only counts fresh bits, so it matches the old `set_insert` boolean.
- **u16 max:** key 65535 gives index 1023 and bit 63, and `index * 64 + bit` stays within `u16`. `domain_words` is capped at 1024 words. A forged key beyond the domain grows `count` to `index + 1`, so it is retained as in the old set.
- **Out-of-range queries:** `contains` and `remove` return `false` for out-of-range keys without allocating.
- **No allocation after refusal (`insert`):** `reserve(0)` runs before any state change, and `storage::filled` runs before `self.words = words`. A failed growth leaves `self` unchanged.
- **`copy_from_slice`:** `count >= self.words.len()` always holds, because `index >= len` and `domain_words >= len`. The slice cannot panic.
- **Trait and inherent `contains`:** the trait impl calls `self.contains(node)`. Inherent methods win method resolution, so it does not recurse.
- **Shift types:** `1u64 << (key % 64)` with a `u16` shift amount compiles.
- **Priority in `live_nodes_with_pending`:** the order matches the old code: payer check, `Nodes::new`, seeds `iter` and `charge`, then `charge` before `reachable`, then `insert`. The `||` short-circuit still skips `insert` for unreachable nodes, and `node == start` is still checked last.

## Uncertainties to verify

1. **Error-priority change in `edge_liveness` (L~126):** `values(payer)` charges only a constant, whereas the old `iter(payer)` on the AVL map may have charged per step or for a stack. I cannot see the old `Set::iter` accounting. If it charged, the refusal point and `WorkCounter` interleaving shift, so confirm this is intended. The new `Nodes::insert` also charges `reserve(0)` per call. Confirm that this matches the old `set_insert` per-call debit.
2. **Dense versus sparse charge:** each loan's first `insert` charges the full `domain_words * 8` bytes, up to 512 B at 4096 points, regardless of live count. The old AVL cost scaled with live nodes. With many loans and sparse live sets, a refusal can now happen where the old code succeeded. This is an allocation-refusal-versus-`charge` priority change, even though no quota was changed.
3. **Growth payment (`insert`):** the new vector is charged in full via `filled`, and the old `self.words` is dropped afterwards with no refund visible here. Whether "old+new growth" means cumulative charging, as intended, depends on `CfgReservation` never refunding on drop. I cannot see that, and the paste has no `Drop` impl, so I cannot confirm the last-`Drop` requirement here.
4. **Uncharged copy work:** the realloc copy is not charged to `WorkCounter`. Forged ascending keys (64, 128, …) can force up to about 1024 reallocs, each copying up to 8 KiB. That is bounded, but not work-counted.
5. **Test helper (`into_vec`):** `cfg_directory::vector` may not exist. The brief mentions `storage::vector`, so check the path. `Keys` has no `size_hint`, so `extend` relies on the precharged capacity being at least `len`. It is, if `len` is consistent with the bits.
6. **`Keys` is not fused:** after returning `None`, further `next` calls keep incrementing `index`. This is harmless unless a caller reuses it.
7. **Sticky and foreign owner:** this is delegated to `CfgReservation::reserve` and `storage::filled`. `Nodes::new` and `values` do call `reserve`, but I cannot verify that authentication happens before the debit or that a foreign owner is rejected.
8. **Missing tests:** `mod tests;` is not pasted. Nothing shows coverage of key 0, 63/64, 65535, duplicates, forged out-of-domain keys, refusal at each reserve point, or excess-capacity sticky failure.

No cap or quota changes are visible in the paste, and no application fit is claimed.
