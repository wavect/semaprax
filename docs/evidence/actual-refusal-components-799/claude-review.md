# claude review (actual-refusal-components-799)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**No concrete blocker found in the supplied source.** The items below could not be settled from the excerpts alone. I did not compile or run anything.

## (1) Authentication

- **Receipt filter.** `unknown_component_receipt` accepts T001 and the T220 `…` is not an in-scope type parameter` message. The applied-`T` message, "type parameter `T` cannot take type arguments", does not end with that suffix, so it is excluded. The `||`/`&&` precedence is correct.
- **Malformed applied-`T` and unsupported nominal types.** `check_declared_type` emits no T001 or absent-name T220 for them. For `T<..>` it emits the applied-T T220 and `continue`s without walking the arguments. A known nominal with the wrong arity yields T221. Neither can become a component refusal, which the two new tests cover.
- **Six-field check.** Equality over code, severity, message, path, span and help is complete, and `position` returns the original's index. If identical duplicate originals exist, the first wins, which is harmless.
- **Field owner.** The owner walk is bounded by `declarations.len()` and uses `fields.contains(field)` for full raw-field equality. For classes it goes from the container up through `extends`. Typing is sound: both or-pattern arms bind the same `&Vec<FieldDeclaration>`, and `Option<&Type>` matches `Some(Type::Named{..})` through default binding modes.
- **Class prefix clones.** If a derived class's own `fields` ever literally contain clones of parent fields, `contains` hits the child first and uses the child's parameter set. A generic parent and non-generic child would then recheck as T001 against an original T220. That fails closed, returning `None` with no withdrawal and no false authority. It is worth a test, but it is not unsound.
- **Method environment (verify).** `callee_component_refusal` builds the parameter set from `function.type_parameters` only. If methods of a generic class `C<T>` do not carry `C`'s parameters in `Function.type_parameters`, the recheck takes the T001 branch against an original T220. It then fails closed and the alias is not withdrawn, so a cascade remains. The visible method test uses a non-generic class. Please add `class C<T>{ fn f(self:C<T>,a:Missing)… }` or confirm the parameters are carried.
- **`Recovery::signature` (possible gap).** The current-function path still gates on `diagnostic.code == "SPX-T001"` and filters `refusal.code == "SPX-T001"`. In a generic current function, an own parameter `b:Missing` produces the absent-name T220, so it never gets a scope entry. Later uses of `b` would see the declared `Missing` type. If that is intended, ignore this. Otherwise it is inconsistent with `unknown_component_receipt`, and the fix is to share the predicate and compare the recheck's refusals by that predicate.

## (2) Ordering of rooted place checks

In `enter.rs`, when `source_place(expression)` is `Some`, the order is `check_source_place_availability` (O101/O107/O109/O110) and then `publish_source_place`. The refusal is withdrawn only afterwards. `source_place` itself requires `record_field_type`, which presumably succeeds for a `Named` Missing field. The next projection's `record_fields` returns `None` for it, so `a.bad.x` falls to `ResumeProject`. Its base `a.bad` is a complete place that goes through availability and then withdrawal, and `ResumeProject` pops `None` and pushes `None`. That path is correct.

## (3) Sibling errors and diagnostic order

The refusal paths in `publish_source_place` and `frame_resume_project` push no diagnostics and do not reorder them. T252, T214 and `reject_native_unit_value` still run before the refusal in `frame_resume_project`. Sibling arguments are separate `values` entries, so their diagnostics are untouched.

One thing to confirm: a refused place now yields `None` rather than `Some(CheckedValue{mode,..})`. If a consumer records "moved" state from the `CheckedValue` and not from the expression's place, a refused-owned place would not be marked moved and a later O109 would be lost. I could not see that consumer. If the move is recorded from the expression, there is no issue.

## (4) Borrowing and typing

- **`source_place_refusal`.** The `Cow` walk is fine. `name` and `field` borrows end before `ty = Cow::Owned(..)`, because the right-hand side finishes using `&ty` before the assignment. This matches the existing pattern in `place.rs`. It requires `record_fields` to return a reference or slice, not an owned `Vec`; `place.rs` already relies on that.
- **Mutable reborrow.** `self.diagnostics` is `&mut Vec<Diagnostic>`. Passing it to a `&[Diagnostic]` parameter coerces, and the borrow is disjoint from `self.recovery` and `self.scopes`.
- **`publish_source_place`.** The `Option<usize>` scrutinee carries no borrow. `place.ty` is moved only in the `else` arm, after `&place` has ended.
- **Privacy.** `Recovery`'s private fields are reachable from the child module. `unknown_component_receipt` is `pub(super)` and is called from `recovery.rs` as `callee_components::…`. `SourcePlace` is `pub(super)` in `place`, which is the same scope as the `pub(in crate::source_verify)` function that takes it, so there is no leak.
- **Visibility to verify.** `types.declarations` is a field read from `recovery::callee_components`. It must be at least `pub(in crate::source_verify)`, otherwise it won't compile.
- **Exhaustiveness to verify.** The four-arm `TypeDeclarationKind` match must be exhaustive. A fifth variant would fail to compile.
- **Authority.** The helpers return only `Option<usize>`, an index into already-recorded diagnostics. They never create or publish a type and never insert `Missing`, so they confer no authority. The scratch vector is discarded.

## (5) `record_traversal::project` shortcut

It returns `Some` only when all of these hold:
- the base is `Var` with a `traversal_row`;
- the element is a non-generic `Named` type that passes `admitted`;
- the declaration is a `Record`;
- the selected field is `String` or `Bytes`, which become `Str` or `SliceU8` as borrows, or `vec_ops::ast_element_is_admitted`, which yields a `Value`.

A field whose type is a missing `Named` (the absent-name case) returns `None` and falls through to the ordinary rooted path, so the shortcut cannot bypass a Missing component. This holds as long as `ast_element_is_admitted` returns `false` for an unresolved `Named` type and `admitted(types, element)` already rejects it. Add a one-line test that a traversal row record with a `Missing` field falls through and is withdrawn. The shortcut checks availability only on the root `Var`, not on a projected path. That behavior predates this change and is not a regression.

## Verdict

None of the five assessed points has a concrete defect. The checks to run before merge are these:

1. Method type-parameter environment for generic classes.
2. T001-only gating in `Recovery::signature`.
3. Move recording for refused places.
4. `types.declarations` visibility.
5. `ast_element_is_admitted` on unresolved `Named` types.

If those hold, there is no blocker.
