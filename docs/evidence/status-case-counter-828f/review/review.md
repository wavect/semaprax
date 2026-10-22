# review review

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

## Findings (source-only; nothing compiled or run)

### Blockers

**B1. The pasted `case_certified.rs` is not the corrected receiver.** It still has `while ordinal < ...len()` with `&...status_sources[ordinal]` (~L38-39) and the "direct while-index" comment (~L36-37). No `for source in &...` appears anywhere.
- If this is the file under review, the corrective was not applied.
- `counter_tests` then compares `counter_before::validate_status_sources_certified` against a body that is textually the same loop. The equality assertion passes vacuously and proves nothing about the `for` form.
- I can only review the `for` form from your description, so it is unverified.

**B2. The "no new iterator" claim is wrong for the `for` form, and it conflicts with your accounting model.** `for x in &vec` desugars to `IntoIterator::into_iter(&vec)`, which creates a `slice::Iter` (two pointers, 16 bytes on the stated 8-byte-pointer host) and a `next()` call per row.
- The `ordinal` reservation (~L29-33) prices only `size_of::<usize>()`.
- Elsewhere you price every simultaneously live pointer (`&Option<CallFact>` ~L48-52, `&CallFact` ~L67-71).
- Under that rule the iterator is an uncharged live carrier, and the `source` reference is also uncharged in both forms.
- Either the model deliberately ignores loop-driver state, in which case the old `while` was equally "free" and the Enumerate worry (~L36-37) was moot, or the `for` form silently shifts the byte frontier.
- Decide which and state it, because a reviewer cannot tell from the source. The test cannot catch this either (see B3).

**B3. The test does not pressure the byte frontier.** `observe` fixes physical capacity at `1 << 24` (~L38 of the test) and varies only `work` over `[MAX, spent, spent-1, 0]`.
- No `reserve_bytes` failure is ever exercised: neither the ordinal 8-byte reserve, nor the per-row pointer reserves, nor the final `CONTROL_BYTES` reserve.
- Four work points do not cover a mid-loop exhaustion at each of the per-row charges (receiving assignment, call assignment, `member`, ordinal increment).
- The `spent-1` case hits only the last charge.
- Sweep `0..=spent` (it looks small) and a physical-limit sweep around `charged()`.

### Likely / uncertain

**U1. Probable new Clippy hit.** `let mut ordinal = 0usize;` sits right before the loop, and `ordinal += 1;` (~L117) runs at body top level. A `for` over a slice with that pattern is what `clippy::explicit_counter_loop` targets, and early `?`/`return` exits don't exempt it.
- The `while` form was presumably chosen to avoid `.enumerate()`.
- The `for` form may trade one lint for another unless there is an `#[allow(clippy::explicit_counter_loop)]` with a justification. I can't confirm this without running Clippy.
- If `ordinal` must stay a separately priced counter, the allow plus a rewritten comment is the honest fix.

**U2. Stale comments and names.** Once the loop is `for`, the comment at ~L36-37 ("direct while-index ... without an Enumerate carrier") and the test name `certified_while_counter_...` are wrong. The test doc says "unchanged counter fees/work", which B2 contradicts.

**U3. `status_sources` type.** `&function.cleanup_plan.status_sources` is `IntoIterator` only if the field is a `Vec`, slice or array. The existing `.len()` and indexing are consistent with that but would also fit a custom indexable type. I can't tell from the pasted source.

**U4. Ordinal semantics of `proof.member(function, source, ordinal, budget)` (~L94).** `ordinal` is the index over all `status_sources`, including propagated-call, allocation and contract rows. If `StatusCases` stores ordinals only over arithmetic rows, `member` would mismatch. The source can't settle this, and it is unchanged by the corrective, so it matters only if the old receiver was never checked on mixed inputs.

**U5. Test coverage gaps.** The fixture (`main`) has only checked-arithmetic and propagated-call rows. It never reaches the `CheckedAllocation`, `ContractFalse`, wrong-lane, `RepeatedStatus`, `UnknownStatus` or wrong-callee arms. It also uses only `Facts::Arena`, so the `Tree` early return is untested.
- Those arms are not changed by the loop rewrite, so this is low risk.
- But "no oracle skipped" holds only for the exercised paths.
- The comparison also discards `sources` (`ids`, `ledger`) after `drop`, so success-path contents are not compared, only the four tuple fields.

**U6. Trait/type assumptions in the test.**
- `assert_eq!` on `Result<(), Diagnostic>` needs `Diagnostic: PartialEq + Debug`.
- `Ok::<_, crate::diagnostic::Diagnostic>` must be the same type as the `Diagnostic` returned by `validate`/`StatusCases::validate`, or `?` needs a `From` impl.
- Your note says the old receiver compiled, so these likely hold, but nothing in the pasted source confirms it for the new function.

### Checked and looks fine
- **Evidence path:** `include_str!` with seven `..` from `.../case_certified/` resolves to the repo root, then `docs/evidence/...`. This is consistent.
- **Module layout:** `mod counter_before;` in a non-`mod.rs` file resolves to `case_certified/counter_before.rs`.
- **Visibility:** `counter_tests` as a descendant can use `super::counter_before` and the `retained`-scope imports.
- **Borrows:** `source: &'hir _` from `function: &'hir ResolvedFunction` is fine, and `&status_sources` shares no mutable state with `budget` or `ids`.
- **Equivalence:** replacing `while`+index with `for` has no semantic difference, because there is no `continue`, no mutation of `status_sources`, and every error path returns before `ordinal += 1`. The ordinal is incremented only after the full fallible body.

**Verdict:** the equivalence is plausible but unverified. B1 (the change isn't in what was pasted) and B3 (the test doesn't exercise byte or ordinal frontiers) should be resolved first, and B2 needs an explicit statement.
