# constructor intrinsic 824 final review (constructor-intrinsic-borrowing-824)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Verdict:** from the pasted source I found no concrete blocker. I found one independence gap and several items I can't confirm without files you didn't paste.

## Parity, which holds for what's shown

- **List `is_list`/`is_step`:** `declaration.as_str() == ID && arguments == [I64]` is equivalent to the old `*ty == resolved_list()/resolved_step()`. `DeclarationId` equality is already `as_str()`, and the pattern needs no `..` because the old code built `TypeParameter` with exactly `owner` and `index`.
- **Step shape (List and Iter):** the boolean chains keep the same order as the old code: id, name, index, fields, then types. The checks are pure, so short-circuit order cannot change the result.
- **`is_*_of_step_parameter`:** these match the old `Nominal{List|Iter,[parameter]}` equality.
- **List `validate_declarations`:** `canonical_step_shape` replaces `step_shape(&resolved_step())`. The dropped `is_step` check is trivially true there.
- **Iter `validate_declarations`:** the `resolved_element_is_admitted_in(I64)` call plus `canonical_step_shape` replaces `step_shape(&resolved_iter_step(I64))`. The dropped `is_step` and `element` checks are trivially true for that input.
- **Errors:** the loop order, the `&&` order and the `SPX-H006` messages are unchanged. The iterator-before-list order is preserved in `original_intrinsics`.
- **Paths:** the `include_str!` relative paths all resolve correctly from `constructor_seed/intrinsic_tests.rs`. The three saved-source SHAs in the test match the headers you pasted.

## Concrete independence gap

`iterator_ops/original_constructor.rs` is not self-contained. Its `step_shape` calls `is_step` and `element` through `use super::*`, so it resolves to the current parent items. The `resolved_iter` and `resolved_element_is_admitted_in` calls also resolve to the current parent. The List copy defines its own `is_list` and `is_step`, but the Iter copy has no local `is_iter`, `is_step` or `element`.

- If production edited `is_iter`, `is_step` or `element` in the unshown parts of `iterator_ops.rs` (outside 94-120 and 395-463), the "original" Iter dispatch silently tracks the new code. The old-vs-current Iter comparison is then partly tautological.
- It also matters semantically. The old `step_shape` admitted any `element(ty)` accepted by `resolved_element_is_admitted_in`, including Bytes and owned-leaf collections. If the current `is_step` was narrowed to a singleton `[I64]`, `step_shape` changes for non-I64 elements. Neither shown test exercises Iter predicates against non-I64 elements. The predicate test only calls the `list_ops` ones.
- **Needed:** the diff for `is_iter`, `is_step` and `element`. If they changed, copy their old forms into `iterator_ops/original_constructor.rs` and update its pinned SHA.

## Unverifiable from the paste

- **Module declarations:** `crate::{list_ops,iterator_ops}::original_constructor::…` needs `pub(crate) mod original_constructor;` in both parents. `original_intrinsics` needs `pub(super)` items and `mod original_intrinsics;`. `vec_intrinsic::reject_reserved_identities` needs `pub(super)` or wider, because `super::super` reaches `validation`. None of these declarations were pasted.
- **`cfg(test)` gating:** if the saved modules are not gated, `original_*`, `is_list`, `step_shape` and `validate_declarations` are dead code in production. If they are gated, check that `resolved_list`, `resolved_step` and `resolved_iter_step` still have non-test callers. Otherwise they trigger `dead_code`, which is fatal if warnings are denied.
- **Fixture admission (speculative):** `mutate` does `variant_cases.get_mut(...).unwrap()` for `ListStep` and `IterStep`. The fixture mentions only `List<i64>` and `Iter<i64>`. Both validators `continue` when a declaration is absent, so nothing here proves the Step declarations are present. If they are injected lazily, the test panics. That would be a test failure, not a production defect.
- **`code == "SPX-H006"` for every mutation case:** this assumes no earlier stage (`vec_intrinsic`, `string_intrinsic`, `stdin_stream`) rejects with a different code. Those stages run in both paths, so parity holds, but the code assertion is unproven.
- **Test API assumptions:** `ResolvedProgram.entrypoint` must be visible to the test module. `Diagnostic` needs `PartialEq + Debug`. `docs/evidence/…` must be inside the packaged crate.

## Observer and funding claims

- **Receivers allocating zero:** the current receivers do only `BTreeMap` lookups, a borrowed `variant_cases` slice, `as_str` comparisons and `DeclarationId::compiler`. I see no allocation on the Ok path. This assumes `DeclarationId::compiler` does not allocate, which test 1 does not prove. The test covers it implicitly, since a nonzero `allocated` fails.
- **`old_physical.allocated > 0`:** this is expected, because the old code builds `vec![I64]` and clones `owner`.
- **Zero-limit refusal:** the third block asserts that `ConstructorSeed::authenticate` fails and refuses at limit 0, with `ok == (false, false)`. That outcome depends on `function_lookup::authenticate` paying for the fixture's three functions, which is outside this diff. It asserts constructor funding behaviour that this source does not establish, and it supports no byte-gap conclusion. Generic-graph and NUL errors stay unpriced.
- **`live == 0` in that block (speculative):** this holds only if `with_limit_usage_refusal` stores `first` without allocating inside `measure`.
- **NUL ordering:** the `bad\0entry` check only asserts parity, not that the NUL error is first.
