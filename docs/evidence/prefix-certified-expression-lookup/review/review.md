# review (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Review of the pasted source only. I ran nothing. These findings come from reading the code.**

## Blockers

1. **The stack carrier is unpaid (prefix.rs:9-36, 46, 58).**
   - `compare` adds a `Result<(Ordering,usize),Diagnostic>` return slot. The slot is at least `size_of::<Diagnostic>()`, so it is a second Diagnostic-sized carrier next to `find`'s own.
   - It also adds the slices `left` and `right`, `end`, `units`, `common`, `a` and `b`.
   - `find` adds `lower`, `upper`, `first`, `last`, `prefix` and the matched `(Ordering,usize)`.
   - `BOUND_UNITS = 4` (line 7) is a byte-pair work-unit constant. Its comment says it covers "selecting/updating the two local bracket-prefix certificates". It cannot cover a Diagnostic-sized carrier, and nothing ties stack bytes to work units.
   - **Smallest inline alternative:**
     - Make the byte loop an infallible `fn lcp_order(l:&[u8], r:&[u8], prefix:usize)->(Ordering,usize)`.
     - Move the `checked_sub`/`checked_add` unit computation and `budget.charge(...)?` into `find`. The only `Result` carriers left are ones the old `find` already had (`charge`'s `Result<(),Diagnostic>`).
     - Then charge the remaining new slots once, before the first read, with no per-compare charge. The nested call's slot is reused, so it overlaps rather than accumulates. Replace the zero in line 44's `reserve_bytes(function, 0, …)` with `size_of::<(Ordering,usize)>() + 4*size_of::<usize>()` (`lower`, `upper`, `first`/`last`, `prefix`).
     - If `compare` stays out of line, charge `size_of::<Result<(Ordering,usize),Diagnostic>>()` plus the same locals instead.
     - A `size_of` sum is a model, not a guarantee about the compiler's real frame. State that explicitly. Do not claim old-header credit.
   - **Check this first:** a positive `reserve_bytes` calls `prepare_builder`. The zero path was written specifically to avoid bootstrapping the refusal `String`. If `prepare_builder` allocates, `hit()`'s `allocation.allocated == 0` (tests.rs:38) will fail. `charge` already calls `prepare_builder`, so this is unclear. Read `prepare_builder` before adopting the fix.

2. **No test reaches the endpoint-miss returns (prefix.rs:54, 64).**
   - The minimum key is `""` (tests.rs:46), so nothing is less than it.
   - The maximum is `"🙂"` (F0…). Every extra query (`"zzzz"`, `"識別"`, etc.) sorts below it.
   - Only the `Equal` endpoint branches (lines 55, 65) run, through the key loop. The `Less`-at-minimum and `Greater`-at-maximum early exits are untested, for both insertion orders.
   - Add queries below the minimum and above the maximum. The minimum needs a fixture without `""`.

3. **Likely test failure at tests.rs:130-137.**
   - The test iterates every function in the fact-owner source.
   - If any function builds an empty arena, `root == NIL` and the loop never runs.
   - `find` then returns `Ok(None)` without charging, and `assert!(first.is_err())` fails.
   - Skip empty arenas, or assert on `arena.nodes.len()`.

## Concerns (correct as written, but not demonstrated)

- **The bracket proof is sound** for byte-lexicographic order.
  - Let `a = lcp(q,L)` and `b = lcp(q,U)`. Then `lcp(L,U) ≥ min(a,b)`, and every key between L and U shares `lcp(L,U)` bytes with L.
  - So every intermediate key shares `min(a,b)` bytes with `q`.
  - Lines 31 and 35 return `common ≤ min(len)`. This also covers the case where one key is a prefix of the other.
  - Equal returns the full length. NUL is just byte 0. UTF-8 `str` ordering equals byte ordering.
  - This relies on `ExpressionId: Ord` being exactly `str` byte order. That impl is not pasted.
- **Prefix-invariant violation is mislabeled.** If `prefix > end`, line 19 yields `None` and reports "work overflow" as a budget refusal. That would hide a logic bug as exhaustion. Use `debug_assert!` or an internal error.
- **New lookup can cost more than old** (+4 per node, plus about two full seed comparisons and the right-spine walk).
  - For trees ≥16 nodes with short keys, or no saving, the budget required exceeds old `find`.
  - A query the old code accepted can now be refused at a tight budget. Line 91's `required` is taken from the new `find`, so this is never tested.
  - Either bound it in a test (for example, new ≤ old + the stated overhead) or call it an intentional semantic change.
- **`saw_saving` (tests.rs:61, 69) is weak.** One saved unit anywhere passes. There is no exact per-query expected-unit recomputation from the descent path and no check that bounds are actually used.
- **Test 2 is tautological (tests.rs:84-85).** It repeats the implementation's formula and uses a mismatch at the first suffix byte. Despite the name, it does not check "actual" bytes read. It charges `end − prefix + 1`, the same upper-bound rate as the old lookup.
- **Mid-codepoint divergence is not clearly covered.** Keys that differ inside a multibyte scalar, such as the same lead byte with different continuation bytes, are not constructed, so the claimed UTF-8 coverage is thin. Add a pair such as `"\u{3b1}"` and `"\u{3b2}"`: both are `CE xx`, so the first byte matches and the second differs.

## Uncertainties (not verifiable from the paste)

- **Paths and signatures in the tests.**
  - `super::super::build::<errors::Legacy>` (line 117) resolves through `arena`'s glob import.
  - The `include_str!` path resolves to `replay/expression_facts.rs`.
  - `with_limit_usage_refusal` returns a 4-tuple (line 130).
  - `ReplayBudget::unobserved().remaining == MAX_REPLAY_WORK_UNITS` (line 39).
  - `Diagnostic.code` is comparable to `&str` (line 99).
- **`arena.first` and `right`-spine maintenance.** Whether `arena.first` is always the minimum is relied on but not shown. Only hit tests exercise it.
- **Paid-before-read** looks satisfied. Each node read (52, 61, 81) follows a charge of 3, and byte reads follow the `compare` charge. Slice headers (16-17) are read before the compare charge, but only after the node charge.
- **Sticky refusal** relies on `charge`'s `refusal_spent` check. The test at line 101 is consistent with it.
