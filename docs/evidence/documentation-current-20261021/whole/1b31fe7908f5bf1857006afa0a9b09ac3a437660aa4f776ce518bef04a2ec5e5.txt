# Review of the pasted source only (nothing executed)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

## Blocker candidates

**1. Flow-insensitive name lookup against a stale entry scope (resolve_statement.rs:236-239; resolve_for.rs:67; byte_ancestry.rs:43).**
- The nested-`for` bypass checks `scope.get(name)` in the outer byte-loop entry scope.
- The pre-scan (`pending`) does not model block scoping. An inner `let data = <Vec or owned record>`, a pattern binding, or an inner `for` binding can shadow a name that is a byte binding in the entry scope.
- In that case line 236 admits the nested `for` and skips the SPX-H006 rejection at 247-255.
- The later `resolve_for` takes the non-byte route. Line 69 only runs `reject_while_disallowed(body)`. As far as the pasted code shows, the nested-traversal rejection lives only in the pre-scan, so nothing re-issues it.
- Net effect: a non-byte `for` nested in a byte loop that used to be refused may now be accepted.
- Unless the source v2 verifier provably forbids shadowing here (not visible), I would block on this. It needs a named negative test.
- The converse is only a false reject: a byte shadow of a non-byte outer name is refused conservatively.

**2. Module visibility (resolve_for.rs:116).**
- `super::resolve_expr::byte_ancestry::reserve_scope()` is called from a sibling module of `resolve_expr`.
- That compiles only if the `mod byte_ancestry` declaration is `pub(super)` or `pub(in crate::hir)`. The declaration is not shown.
- `reserve_scope` is `pub(in crate::hir)`, but a private parent module would still hide it.
- Confirm the declaration. It is a compile blocker if it is wrong.

## Accounting concerns

**3. Possible double or over-charge (byte_ancestry.rs:19-24, 27; resolve_for.rs:116).**
- The doc says the new handle has "no new heap/control-block allocation", yet `reserve_scope` charges `size_of::<Scope>()` for every byte-loop frame push.
- The ancestry slot, `Option<(Scope, usize)>`, is already inside `size_of::<Self>()` charged at `acquire`.
- The frame-resident handle is covered by the actual Frame layout and census, plus the const assert at resolve_expr.rs:47.
- At most one handle is live in the frame and one in `outer`, because line 67 moves `scope` rather than cloning it.
- So the charge is either redundant, which contradicts "exact", or it covers something the comment does not name. Sibling byte loops also re-charge each time.
- Clarify or justify it, and write the exact byte arithmetic in a test.

**4. Charge release (byte_ancestry.rs:9-18).**
- It is unclear whether `reserve_active_required` is cumulative or released.
- `acquire` runs once per `resolve_expr_iterative` call, including every closure body, and adds the `Result<Self, Diagnostic>` size.
- If charges are never released, nested closures and many loops accumulate.
- That is not visible here, so I can't call it wrong.

**5. Error-order shift (resolve_for.rs:115-117 vs. resolve_expr.rs:65-68).**
- The storage refusal now fires at push time, before the loop-depth check that `enter_loop` performs at pop time.
- A depth-limit error that previously surfaced first could be pre-empted by SPX-H006 "storage refused".
- Also, line 65 (`element`) runs before the scan at line 67. Confirm this matches the old order.

## Semantic observations (likely intended, but unproven)

- **Ancestry lifecycle (byte_ancestry.rs:30-41; resolve_expr.rs:65-79).** The depth arithmetic checks out. `enter_loop` increments first, so `enter` records the post-increment depth. `LeaveLoop` decrements first, so `leave(d-1)` clears only when the outermost byte loop exits.
  - A non-byte loop between byte loops does not clear it.
  - Sibling byte loops re-enter correctly.
  - The first-wins rule is deliberate, but it means the outermost *byte* loop is the entry, not the outermost loop. This depends on the v2 policy.
- **Inner scope ignored (resolve_for.rs:67).** `byte_entry.or(Some(&scope))` means an inner loop's own scope never participates. A nested `for b in inner` whose `inner` is bound inside the outer body is therefore refused, because the name is absent from the entry scope. That is sound but possibly over-strict, so it should be pinned by a test.
- **Pre-scan order (resolve_statement.rs:230-246; resolve_expr.rs:1458-1467).** The LIFO pushes (body, then condition/values) give left-to-right scanning. The While arm and the For arm are consistent. At 1458-1467, condition-before-body is kept.
- **Redundant rescans (1455-1467).** The outer pre-scan already traverses nested While bodies, and each nested While rescans its body again. That is O(depth × size). The charging of `pending` growth is not visible in the snippet.
- **Return handoff skip (resolve_statement.rs:263-269).** This contradicts the "compiler still scans all source expressions" claim only if `source_named_handoff` can accept something other than a bare name. Verify it is name-only.
- **Closures (272).** Skipping closure bodies is consistent with the fresh `Ancestry` per invocation. Any `for` over outer bytes inside a closure is then strictly rejected, which is conservative.

## Rust typing uncertainties

- `&***scope` (byte_ancestry.rs:43) requires `Scope` to deref twice to `BTreeMap<String, Binding>`, for example `Rc<Cow<'a, BTreeMap<…>>>`. The definition is not shown, so I can't confirm it.
- The "no new allocation" claim in the doc comment at lines 19-21 holds only if `Scope::clone` (resolve_for.rs:117) is an Rc refcount bump rather than a map copy. Confirm that.
- `Frame<'static>` in the const assert vs. `Scope<'a>` in `Ancestry<'a>`: the lifetime variance is plausibly fine but unverified.
- The 592-byte bound with the new `EnterByteLoop(ControlTarget, Scope)` variant is only compile-checked, so it can't be assessed here.

## Tests

Only the import header of the test module is visible (byte_ancestry.rs:47-53). I can't confirm any behavior is covered. The following cases need explicit tests:
- inner shadowing by a Vec, owned record, or mutable binding (finding 1);
- clearing on exit of the outer loop, and sibling loops;
- a non-byte loop in between;
- a closure body getting a fresh ancestry;
- refusal at `acquire` and at `reserve_scope`, with exact charged bytes and the no-leak-on-error path;
- error order: condition before body;
- Vec, record, and owned traversal staying strict with the contextless receiver.
