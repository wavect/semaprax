# Residual constructor acquisition audit: List/Iter prelude structural checks

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

This is from the pasted source only, with no tools, edits or tests run. Items marked **unseen** depend on code that wasn't pasted.

## Findings

**1. `list_ops::is_list` and `is_step` allocate per call.**
- Each call builds `resolved_list()` or `resolved_step()`, which is a `Vec` of one `I64` plus a `DeclarationId::compiler(..)`.
- It compares with `==` and drops the result.
- `step_shape` then calls `is_step(ty)`, builds `owner`, clones `owner` into `parameter`, and builds a second `Nominal { List, vec![parameter] }` just to compare against `tail.ty`.
- `validate_declarations` also builds `resolved_step()` only to hand it to `step_shape`, which immediately rebuilds an identical one.

**2. `iterator_ops::is_iter` and `is_step` are already borrowed matches.**
- `element()` is a slice pattern and allocates nothing.
- The only discarded construction there is `resolved_iter(parameter)` in `step_shape`, plus the `parameter` clone.
- `validate_declarations` builds `resolved_iter_step(I64)` as the argument. I'd leave that alone, because the `resolved_element_is_admitted_in` gate is **unseen** and I won't assume it holds for `I64`.

**3. Hostile argument trees: `is_list`/`is_step` do not walk them.**
- This assumes `ResolvedType: PartialEq` is derived, which is **unseen**.
- The right-hand side is depth 1: `[I64]` or `[TypeParameter]`.
- Derived `Vec`/enum equality compares length, then element 0's discriminant, and returns false on mismatch.
- So a hostile left operand such as `List<List<List<…>>>` costs O(1) per call. There is no recursion and no stack risk.
- The only cost is the canonical-value allocation and drop, so the borrowed form changes allocation behavior, not asymptotics.
- The one real walk of a hostile element is `resolved_element_is_admitted_in` in `iterator_ops::step_shape`. It is **unseen** and unaffected by this proposal.

**4. Out of scope.**
- `generic_template::validate_call_graph` clones ids into a `BTreeSet`, a `BTreeMap` and an `incoming` map. That is the separate generic-graph path.
- Nothing here funds it, and I make no whole-funding claim.
- `authenticate` and `authenticate_shared` duplicate the same prelude sequence, but only one runs per seed.

## Equivalence preconditions

Both must be pinned by a test:

- `DeclarationId` equality is string equality.
- `DeclarationId::compiler(List).as_str() == LIST_ID`, and likewise `ListStep` against `STEP_ID` in `list_ops`.

The pairing in `list_ops::validate_declarations` and the existing `as_str() == ITER_ID` use in `iterator_ops` both suggest these hold, but I haven't seen the definitions.

## Proposed replacements

The old builders (`resolved_list`, `resolved_step`, `resolved_iter`, `resolved_iter_step`) stay intact. They are still used for construction elsewhere and serve as test oracles.

### `src/list_ops.rs`

```rust
fn is_step_parameter(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::TypeParameter { owner, index: 0 } if owner.as_str() == STEP_ID)
}

fn is_list_of_step_parameter(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == LIST_ID
        && matches!(arguments.as_slice(), [argument] if is_step_parameter(argument)))
}

pub(crate) fn is_list(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == LIST_ID
        && matches!(arguments.as_slice(), [ResolvedType::I64]))
}

pub(crate) fn is_step(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == STEP_ID
        && matches!(arguments.as_slice(), [ResolvedType::I64]))
}

pub(crate) fn step_shape(index: &crate::hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    is_step(ty) && canonical_step_shape(index)
}

fn canonical_step_shape(index: &crate::hir::DeclarationIndex) -> bool {
    let owner = DeclarationId::compiler(CompilerDeclaration::ListStep);
    let Some(cases) = index.variant_cases(&owner) else {
        return false;
    };
    matches!(cases, [nil, cons]
        if nil.id.as_str() == NIL_CASE_ID && nil.name == "Nil" && nil.index == 0 && nil.fields.is_empty()
        && cons.id.as_str() == CONS_CASE_ID && cons.name == "Cons" && cons.index == 1
        && matches!(cons.fields.as_slice(), [head, tail]
            if head.id.as_str() == HEAD_ID && head.name == "head" && head.index == 0 && is_step_parameter(&head.ty)
            && tail.id.as_str() == TAIL_ID && tail.name == "tail" && tail.index == 1
            && is_list_of_step_parameter(&tail.ty)))
}
```

In `validate_declarations`, replace the call inside the `else` branch:

```rust
&& canonical_step_shape(&program.declarations)
```

This is equivalent because `resolved_step()` always satisfies `is_step`, so the old call reduced to the shape check. The old `ty` argument was otherwise unused.

### `src/iterator_ops.rs`

Only the `parameter` and `rest` comparisons change. `is_step`, the admission gate, `owner`, and the `validate_declarations` call stay as they are.

```rust
fn is_step_parameter(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::TypeParameter { owner, index: 0 } if owner.as_str() == STEP_ID)
}
fn is_iter_of_step_parameter(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == ITER_ID
        && matches!(arguments.as_slice(), [argument] if is_step_parameter(argument)))
}
```

In `step_shape`, delete `let parameter = …;` and use these two replacements:

- `item.ty==parameter` becomes `is_step_parameter(&item.ty)`.
- `rest.ty==resolved_iter(parameter)` becomes `is_iter_of_step_parameter(&rest.ty)`.

The iterator change assumes `resolved_iter(e)` is `Nominal { compiler(Iter), vec![e] }`, which is **unseen** (defined near line 115). Confirm it before applying.

## Focused tests

Keep the old bodies in the test module as oracles. Oracle names below are `old_list_is_list`, `old_list_step_shape` and `old_iter_step_shape`.

1. **`is_list`/`is_step` differential.** For each type, assert `new == old` for both `list_ops` predicates. The type set is:
   - `List<I64>`, `ListStep<I64>`, `Iter<I64>`, `IterStep<I64>`
   - `List<>`, `List<I64,I64>`, `List<I32>`, `List<List<I64>>`
   - `List<TypeParameter{ListStep,0}>`
   - `Nominal` with a foreign declaration and `[I64]`
   - `TypeParameter`, `Function`, and `I64` itself
2. **Hostile arguments.** Wrap `I64` in a `List<…>` nest of about 1000 levels and in a wide argument vector.
   - Assert results match the oracle and are false.
   - Tear down the tree iteratively, or keep the depth moderate, so the test's own `Drop` doesn't overflow.
3. **Identity pin.** Assert `DeclarationId::compiler(List).as_str() == LIST_ID`, and the same for `ListStep` against `STEP_ID` in `list_ops` and for `Iter`/`IterStep` in `iterator_ops`. Also assert that equality of two compiler ids with the same string holds.
4. **`step_shape` mutation matrix (list and iter).**
   - Start from the canonical index, then mutate one field at a time. In each case, assert the new result equals the oracle.
   - Mutations:
     - `head.ty` index 1
     - `head.ty` owner swapped between the List-step and Iter-step ids
     - `tail`/`rest` with `I64` instead of the parameter
     - `tail`/`rest` with two arguments
     - `tail` declaration swapped between `List` and `Iter`
     - wrong case name, index or id
     - extra `nil` fields
     - a third case
   - Also assert that a missing variant returns false.
5. **Diagnostics.** Run `validate_declarations` on the canonical program, expecting `Ok`.
   - Then run it on each mutated program from item 4, expecting `SPX-H006` with the same message strings as before ("immutable list prelude declaration is not canonical" and "iterator prelude declaration is not canonical").
   - Check that the declaration iteration order and the first-failure choice are unchanged.
6. **Argument-independence.** For `list_ops::step_shape(index, &ty)`, assert that `ty` non-`ListStep<I64>` still short-circuits to false before any index lookup. A test double that counts `variant_cases` calls would show this, if one is available.
