# OPT812 held generic profile boundary

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

This is source-only design evidence for [OPT812](https://github.com/wavect/semaprax/issues/812), not an implementation or a G171 improvement receipt. No Rust build or test ran. The attempted production memo was rejected during review and removed completely; the current classifier, caller, independent original receivers and source joins remain unchanged. `source-review.json` binds the inspected private source bytes and the actual bounded Claude review.

## Why the parameter runtime memo was rejected

A retained call target can lend its complete immutable parameter type after the existing canonical lookup (`src/hir/validation.rs:2699`). Its existing function/instance ordinal and parameter ordinal can seal program, index and full argument-tree membership without another directory scan. A bounded pointer-key memo can therefore avoid recursively owned keys. That authenticates identity; it does not establish a useful result to cache.

For ordinary generic record parameters, `owned_text_record/paid.rs:114` refuses the runtime shape before acquiring pending/active backing. The collection fallback in `declared_collection/paid/early.rs:36` also requires an empty argument list before acquiring backing. The special map collection case at `paid.rs:80` is an exception to a blanket generic-false claim; it already takes the earlier collection branch in `validation/call_parameters.rs:181`. Existing vector carriers likewise use their earlier branch. The proposed new cache would retain cheap false runtime observations at the remaining argument-ownership branch, acquire an additional paid Rc carrier, and could refuse a call that the original branch accepted. No measured saving justifies that acquisition. Its proposed 17-slot bound was a retention limit, not a replacement semantic admission limit; that distinction does not repair its lack of value.

The expensive generic scan is `paid.rs:158` (`contains_string`): complete structural visited keys, field substitutions, ordered traversal and funded backing. It can return false for admitted generic types containing no owned text. The proposed cache did not cover that scan.

## Existing equality does not authorize an earlier cache hit

`validation/callable_types.rs:9` performs the profile check at the start of `validate_type`. The expression entry in `validation.rs:2304` calls that method before descending into the expression. Its call-argument full equality check is later, at `validation.rs:3282`, after the argument subtree has completed. Thus the equality witness does not dominate the costly profile check. Using only the retained parameter type to skip validation of the argument's separate owned type would trust hostile HIR without proof. Retaining a raw pointer to a detached/generated expression type would also exceed the sealed parameter token's lifetime authority.

A future equality-witness design must preserve the exact comparison once, full argument-tree equality, existing diagnostic selection, all per-expression type/lifecycle/effect checks and sticky budget authentication. If it moves the pure comparison earlier, it must prove that malformed identities, expression/profile failures, subtree failures and eventual type mismatch select the same complete diagnostic and refusal in the original order. It must not introduce an additional uncounted structural comparison. Current source does not supply that proof or a paid work permit for the new directory/membership work. This is a broader continuation design, not a safe change at the current argument ownership seam.

## Concrete next owner boundary

The genuinely repeated held-parameter work at the current seam is type facts and ownership shape queries, including `borrowed_type_facts` (`validation/nominal_facts.rs:197`). Generic facts currently fall back to the existing owning declaration-index route. Funding and caching those queries must be coordinated with the declaration-index paid facts owner; this evidence does not add authority to its ordinary path. A parameter-only runtime cache should remain absent.

A production implementation needs an admitted generic fixture with actual repeated expensive work, exact original/current diagnostic parity, equal-index and equal-ID/different-program negatives, full nested argument identity, finite capacity eviction/fallback, sticky hit checks, and allocator observation through receiving Drop. It must show lower actual work/debit rather than merely more hits. No acceptance or issue closure is claimed here.

The existing executable controls remain `hir::validation::executable_profiles::tests::generic_callable_and_unknown_types_stay_uncached_and_exact`, `::memo_matches_original_verdicts_across_indexes_with_equal_ids_cold_and_warm`, `::warm_hit_is_zero_debit_allocation_free_and_still_sticky`, and `::actual_validator_clone_and_nested_constructor_share_exact_index_memo`. They are unchanged and were not run by this lane. There are no new owning selectors to add for this design-only commit.
