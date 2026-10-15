# Review of the OPT780 proposal (text only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The pasted text has no line numbers, so I cite section headings. I could not check any claim about the existing code against source.

## Blockers (the spec must resolve these before code)

1. **Admission order versus type information.** "Source admission" says the current `while_rules` rejects every nested For/ForBorrow/ForOwn before typed body checking. It also says byte-ness is decided by checking the source "in its current ordinary scope", and that "non-byte entry behavior and diagnostic order" stay intact. Deciding that a nested source is an immutable `Slice<u8>` needs type and scope information. The old rejection fires earlier, so one of two things must happen:
   - the new check runs earlier than before, which changes the order for non-byte inputs; or
   - the old diagnostic is deferred, which also changes the order.

   The proposal needs to state which one it does and add a differential test on old non-byte nested inputs. As written, "intact" is unachievable without that decision.

2. **Standalone replay has no payer.** "Independent HIR proof" requires public wrappers to do "complete checked replay" when no census is held. Replay needs the same scratch: frames, the counted index and ordered identities. The accounting paragraph says legacy no-owner paths stay "unqualified", and every new carrier must use the "receiving payer before construction". These conflict for the standalone path. Either replay is allocation-free or has an explicit payer, or the standalone API is declared unaccounted. The proposal must pick one and say how replay avoids O(nodes × depth) per call, since every wrapper call with no census redoes a full-function walk.

3. **Census binding to function and context is unspecified.** "No source spelling, cached bool, pointer by itself, hash or ID alone is authority", yet the replacement is "ordered borrowed identities with exact expression-reference checks". Test 4 expects a "foreign function/closure census" to be refused. The proposal never says what a consumer compares against. It needs a concrete rule, such as a function identity plus root-expression reference checked by each of `lexical_exit`, `cleanup_plan` and `native_emit`. Without it, "must NOT silently switch to shape-only" is only a slogan.

4. **Mixed-loop closure is incomplete.** Mixed Vec/row/own nesting is refused "in either direction", and a nested `while` must not be a loophole. The proposal does not say what happens in these shapes:
   - a byte-for inside a `while` inside a byte-for (admitted or refused?);
   - a byte-for inside a `while` inside a Vec-for (should be refused through the intervening `while`);
   - a byte-for inside a closure or nested function inside a byte-for ancestry.

   It needs a rule that any non-byte source-for ancestor, at any distance through `while`, refuses. It also needs fixtures for each of these shapes. The current test list names only "mixed Vec/row/own refusal".

## Tensions to resolve

- **"No new depth cutoff" versus recursive lowering.** The admission section forbids an arbitrary depth cutoff and the source walk is iterative. The lowering section says "reuse… receiver lowering recursively". The interpreter, C11 and Wasm emitters may then recurse to a depth bounded only by expression-depth limits. Either show that those limits bound recursion safely or require iterative lowering. Add a deepest-admitted-nesting test on all backends.
- **Shadowing.** "Existing shadowing/rebinding diagnostics remain exact" sits beside "Assignments/rebindings/shadowing against ANY ancestor are rejected". The proposal does not say which diagnostic fires when an inner item reuses an outer item's name, or whether that case is already covered by the old one.
- **Old nested refusal.** Test 3 says "retained old nested refusal", but nested byte loops are now admitted. That refusal can exist only in the `cfg(test)` v1 oracle, which is test-only code and not production evidence. Say so, and say how drift between the copied scanners and production is detected.

## Implementation requirements (not contradictions)

- **Borrowed census in owning contexts.** If a census borrowed from the HIR is stored beside the owned per-function context, that is a self-referential struct. Pass the census by reference through the call chain, or tie it to a lifetime parameter.
- **Counted frozen-ValueId index.** Every push and pop must be charged to the existing work units. Duplicate-identity rejection must be specified, since "where necessary" is vague.
- **Same-root loans.** Nested same-root freezes consume more loan, point and edge budget. Cap and one-short controls must run at nested depth, not only on the old inputs.
- **Structural paths and backend names.** Receiver paths must be keyed by authored-body path, not by root or ValueId. Native counter/length symbol names and Wasm locals must be unique per nested level. Same-root nesting makes a collision most likely.

## Uncertainties (cannot judge from the text)

- **tag3 sufficiency.** The claim that BorrowBytes tag3 "already expresses all levels" is asserted, not shown. The decoder or checked-cache replay may still refuse nested regions even if the encoder is fine. Needs a decode-then-replay hostile test.
- **Stale cache outcomes.** If no wire bytes change but nested programs flip from refused to admitted, any persisted diagnostic or outcome from before the change could replay as the old refusal. Whether the Project cache key covers semantic admission version is not stated, and nothing in the proposal settles it. Test 6 "warm vs cold" only helps if the warm entry was written by the old compiler.
- **Graph and workspace joins.** "Independently reconstructed workspace call/type edges must match exact nested identities". It is unclear whether those edges carry anything today that distinguishes nested regions.
- **Same-typed swaps.** The hostile cases cover swapped receiver and provenance. Test 4 does not cover a swap between two different nesting levels of the same function, which a counted index could mask.
