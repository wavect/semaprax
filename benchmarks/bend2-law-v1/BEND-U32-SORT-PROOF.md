# Supplemental all-finite-U32-list Bend source proof

The pinned Bend revision `947db722640c86247849343657bf2f7ef01cb7f1`
accepts `fixtures/full-u32-encoding-v1/sort-universal.bend` with actual
`--verdict`. Its imported `sort.bend` is unchanged, including the actual
`U32.is_le` Boolean dispatch in insertion. The retained elaboration also
passes a direct invocation of the locally provisioned BendTT kernel.

The certificate contains two universally quantified claims:

- `sorted_sort(xs)` proves `SortedB(Sort.sort(xs), 0)` for every finite
  `List<U32>`. `SortedB` requires each element to be at least the previous
  element, beginning at unsigned zero; no length bound is imposed.
- `count_sort(probe, xs)` proves that `Sort.sort(xs)` and `xs` contain exactly
  the same number of copies of `probe`, for every `U32` probe and every finite
  `List<U32>`. The count is `Nat`, so multiplicities cannot wrap.

`ULE` uses the same `Word.cmp` as `U32.is_le`. The proof establishes comparison
reversal by word induction and uses that to return order evidence together
with an equality to the exact Boolean branch chosen by the implementation.
Equality transport connects insertion preservation to that branch. A separate
word induction establishes zero as a lower bound. The insertion and sorting
proofs then proceed by list induction. The count proof also follows the
actual Boolean insertion branch and commutes two count increments; it never
replaces the implementation with a model sort.

## Why the count theorem establishes permutation

Here permutation means equality of finite multisets, including multiplicity.
The universally checked count equality is exactly this extensional definition.
It also implies the usual deletion/reordering definition: induct on the input
list. If the input is empty, an element of a nonempty output would have positive
output count and zero input count, a contradiction. Otherwise write the input
as `h :: t`. Equality at probe `h` implies that the output contains `h`.
Remove one occurrence to obtain a finite list `r`. For probe `h`, both counts
decrease by one; for every other probe neither count changes. Thus `r` and `t`
have equal counts for every probe. Apply induction and reinsert the removed
`h` at its original output position. This is a mathematical consequence of
the checked theorem, not a separate Bend theorem about an inductively defined
`Permutation` datatype.

## Retained negative evidence

`law16_bend_u32_sort_proof.py` derives an empty-output mutant from the exact
candidate. It removes the five concrete fixture controls and uses only the
unchanged count proof section, so a finite example cannot mask the intended
universal-law rejection. The actual `--verdict` invocation exits 1 with
`SOME PROOFS FAIL`, specifically at `count_sort`: its target starts with a
zero output count while its offered proof still starts with insertion.
This is a front-end proof/type rejection before kernel replay, not a kernel
counterexample-search result.

The separate `empty-witness.bend` passes `--verdict` proving that the mutant's
output count of `1` on `[1]` is `0n`, while the input count is `1n`. These
incompatible counts make the failed universal statement explicitly false;
the negative result is not merely a failure to find a proof.

## Replay and provenance

```sh
python3 benchmarks/bend2-law-v1/law16_bend_u32_sort_proof.py \
  --bend-root /path/to/pinned/bend2 --bun /path/to/bun \
  --output-dir /path/to/new/evidence-directory
python3 benchmarks/bend2-law-v1/law16_bend_u32_sort_proof.py \
  --verify benchmarks/bend2-law-v1/evidence/bend-u32-sort-universal-v1/capsule.json
```

The runner requires the pinned checkout's exact checker, elaborator, compiler,
CLI, Base and kernel source bytes. It records Bun and kernel executable hashes,
checks that sources and tools do not drift during execution, and retains raw
stdout/stderr for candidate verdict, empty-sort rejection, proved empty-sort
counterexample, elaboration export, and direct kernel replay. The retained
elaboration is source-associated by this local run. Its kernel binary/source
association is a local cache association, not a reproducible build attestation.
Review of retained files verifies their hashes; physical replay requires the
provisioned external toolchain.

This adds a supplemental Bend source theorem. It does not admit or modify the
original v1 task manifest, certify the distinct SEMAPRAX `law16.*` declarations,
or create matched cross-language timing, runtime-lowering, or platform evidence.
The existing LAW15 all-i64 source theorem has separate declaration identity.
