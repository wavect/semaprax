# review (design-review-two-options)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Review of pasted snippets only. Nothing was executed, and none of this is source or runtime approval. The snippets have no line numbers, so I cite function and snippet names.**

## Verdict
**Option A, with a narrower scope than "any independently safe composition."** B encodes origin and does not appear to close an authority hole.

## Blockers (concrete, from the pasted code)

1. **Subtree-authorization bypass in `frozen_body`.** The walker returns `Ok(authorized)`, and the nested-receiver rejection is the only thing stopping the outer checks from being skipped. The easy way to admit a nested receiver is to authorize or skip the inner subtree. That would exempt the inner authored body from the outer rules:
   ```
   for x in v { for y in w { <assign outer counter / rebind v / use outer payload> } }
   ```
   The inner receiver must be admitted only for its exact lowered shape. The walk must still apply the outer `payload`, `frozen`, and shadowing checks to the inner body and to the inner's lowered Let/Assign statements. Add a test where the inner body mutates each outer frozen ID.

2. **Contextless consumers will reject or misjudge nested programs.** `draft_bytes` calls `crate::hir::byte_traversal_region(expression)`, and the wrapper calls `validation::byte_traversal::region(region)`. Both are contextless. If contextless stays strict, which is right, every accepted nested program fails at the first consumer. That is a closed failure and an integration blocker. If contextless is relaxed instead, an inner receiver would be accepted without proof of its outermost-entry binding or ancestor freezes, which is unsound. Both consumers must take the borrowed census, or nested programs must stay unsupported.

3. **Freeze release granularity.** The source freeze IDs hold, but I cannot see the loan-plan kill logic. `draft_bytes` makes one `TraversalFreeze` per site, with `seeds` as a singleton After node. Liveness is therefore structural, not use-based. The test below must pass before any widening:
   ```
   let v = view(r); let w = view(r);   // same root
   for x in v { for y in w {}  r.push(0) }   // push after inner exit
   ```
   This must be rejected. If inner exit clears by root instead of by loan or site, it is accepted. The test should also cover the loop back edge and `v`/`w` swapped across levels.

## Uncertainties
- `nested_byte_source` receives only `values`, and the gate pushes only `body`. It is unclear whether it compares binding identity against the prescan set or only checks type and name. A body-local `let v = …` that shadows a preexisting view must be rejected as not present at outermost entry. It is also unclear whether the inner loop's item and counter are registered for deeper scans.
- `frozen_body` checks Let/Assign only inside `Block` and `VecConstructor`, plus match-variant shadowing. Other mutation forms, such as `&mut` call arguments, must be caught by loan or mutation checks. Nesting does not change this, but it should be tested at depth 2.
- With a latch emitted "only if lexical exits," an inner `continue` or `break` could attach to the wrong loop's `SourceForStep`. This case needs a test: the outer loop has no lexical exit, and the inner loop does.
- Nesting makes work O(n·m). If any static cost summary assumes one traversal level, it needs checked multiplication. Dynamic fuel is presumably fine. I cannot tell from the snippets.

## Answers

**1. Which option.** A is minimal and sound. If the descriptor is removed, the loop is still recognized by `receiver_shape`, so it still hits the frozen checks. Failure is closed because of the shape check, not a tag. A tag would add no authority.

**2. Are mixed-source rules distinct from HIR safety?** Yes. "Byte-for only under byte-for" is profile policy and belongs in the typed-source gate, which already has it. HIR must prove only these things:
- Each receiver is exact and complete, with an immutable view that is authenticated and bound outside the outermost region.
- Every enclosing view root, counter, item, and payload is frozen for the whole child body.
- Nearest-target exits resolve to the right loop.

Nonlexical Copy Vec source-for lowers to an ordinary While, so it is the same as a handwritten loop. It is already outside `receiver_shape`, so no new ban is needed.

**3. Counterexamples.** Blockers 1 and 3 are the candidates. Blocker 1 is the likelier to be real. I have no concrete counterexample that A's design must fail on, only these two implementation risks.

**4. Fail-closed checks (no broadening).**
- **Ancestry:** a raw receiver inside a byte body is admitted only if its nearest enclosing loop is that byte region, with if/match/block allowed in between. A byte receiver inside an intervening manual While, row loop, or own loop stays rejected. This does not ban manual loops. It only declines to prove nearest-target behavior across a foreign loop.
- **Row/own:** any non-`BorrowBytes` `SourceForStep` or row/own descriptor inside a byte body is rejected by descriptor. A byte receiver inside a row/own body keeps the old rejection. Their cleanup and latch proofs are untouched.
- **Standalone `region()`:** it keeps the old strict result. An outer region containing a nested receiver is rejected. An inner region must not claim ancestor freezes, so it returns no proof. It must never be cached or reused as context-bearing evidence. Its API should be labelled unqualified.
- **Census:** it is bound to exact context, declarations, function, execution, and region refs, and is never persisted. Stale or mismatched refs make it fail. Each new carrier is funded on the active owner before allocation.

**5. New serialized fact?** Not for unchanged primitive shapes, provided all of these hold:
- Outermost-entry binding and view provenance are rederivable from HIR and declarations. Global validation already rederives provenance, so this looks achievable.
- No backend (interpreter, C11, Wasm) or cost model keys behavior on origin.
- No persisted artifact is consumed without rerunning the source gate while still relying on the "mixed source rejected" policy for something other than HIR safety.

A new fact is needed only if one of these fails. If a consumer needs source origin, treat it as a language contract, not memory safety. Then it needs its own justification, not a mixed-source tag.
