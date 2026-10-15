# full walker discovery (ownership-clone-census)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I reviewed the pasted code only (no tools run), so every "verify" below is a claim I couldn't check from these files.

## The 514-frame census: inline accounting, not a heap deficit

`clone_payload_bytes` allocates nothing. `[None; 514]` of `Option<(&ResolvedType, usize)>` is about 8.2 KB of stack, so no heap payment is missing, and `Payer::clone_type` pays the right heap bytes for the clone that follows. The stale comment is the only inconsistency: nothing in `ownership_flags` reserves a "clone cursor", so "callers reserve setup" is false for this payer. The real issues are three:

1. **Unpriced 8 KB zero-init per call.** `Storage::clone_type` runs for the root, every type-parameter argument, and every substituted leaf (`substitute.rs` `leaf =>` arm). A childless type such as `I64` still pays the 8 KB memset and the work loop, and the heap payer charges 0 for it. The cost scales with the number of leaves and fields, and no byte limit prices it.
2. **Possible frame inflation.** If `clone_payload_bytes` is inlined through the generic `Payer::clone_type` into `compute_paid_type_facts`, that function's frame grows by 8 KB for its whole lifetime. The fallback is reachable from `validate_argument_ownership_view` at arbitrary expression depth, so this adds to the deepest validator stack.
3. **Unverified stack control.** I can't see whether the validator's depth control prices the deepest leaf chain (classifier → key → clone census).

**Minimum control**, with no cap or waiver:
- Return `Some(0)` at the top of `clone_payload_bytes` for childless types, before the array exists. That covers all leaves, `TypeParameter`, `Nominal` with empty arguments, and `ArrayU8`.
- Mark the array-owning body `#[inline(never)]` so its frame is only live during the call.
- Fix the comment.
- Add one stack-high-water test at maximum validation depth that reaches a deep generic fallback.

## Concrete findings

**1. Depth and work refusal is reported as a layout overflow and changes behavior.**
- `clone_payload_bytes` returns `None` when the type is 514 or more levels deep, or when `MAX_REPLAY_WORK_UNITS` runs out. `pay(None)` reports that as "ownership type flags layout overflowed" with `requested = usize::MAX`.
- The original `ty.clone()` had no such limit, so a valid deep type is now refused.
- Verify two things: whether the parser or resolver limit on type nesting is below 514, and whether `charge::failure` records the sticky first refusal.
- If a `pay(None)` refusal isn't sticky, any caller that drops the `Err` sees a later `authenticate()` succeed again. The existing test only exercises sticky behavior via `reserve_active_required`.
- Fix: a distinct message for the depth/work case, plus boundary tests at depths 513, 514, and 515 through `Payer::clone_type`.

**2. Possible heap under-count in the census.**
- The census counts only argument vectors and the `Function` result box. `Nominal.declaration` and `TypeParameter.owner` are counted as 0.
- That is correct only if `DeclarationId::clone` is refcount-only. `call_parameters` counts `ValueId` by `as_str().len()`, which suggests string-like ids exist in this codebase.
- If `DeclarationId` owns a `String`, every nominal clone is under-paid.
- Verify the type. Then add an exact test: observe allocations around one `Payer.clone_type` on a nested `Function` and `Nominal` type and assert `allocated == clone_payload_bytes()`.

**3. Cache precedence on a declined census for zero-argument nominals.**
- `cached_facts` always wraps the zero-argument case in `Some(...)`, so it can't signal "declined". `flags_paid` claims "declined census keeps the owned-key fallback", but only the compound arm implements one.
- A zero-argument decline therefore becomes a miss and goes to `compute`, which skips a forged or real entry at `nominal:<len>:<id>:0:` that the original `type_facts` would have hit.
- This matches the existing `type_flags`, so it's parity with that function, not a new regression. It is still a divergence from the original if `nominal_key::get` can decline.
- Verify its decline conditions. If it can decline, make it return `Option<Option<_>>` like `compound_get`.

## Test gaps

- **Weak bound.** `wide.allocated <= required` is only an upper bound, so slack from other payments can hide an under-count. Test `clone_type` on its own for exactness (finding 2).
- **Narrow fixture.** The generic fixture is only `facts.generic<I64>`. It never covers `Function`, multi-argument or deep nominals, or wide field lists, and nothing drives the deep-stack path.
- **Missing forged-cache cases.** Forged entries are covered for the zero-argument nominal and `I64`, but not for a compound key, `ArrayU8`, or `Function`. Add those to the `compare` fixtures to pin the compound-arm precedence.

## Checked and fine

- **`Payer::grow`:** it pays the full new capacity before reserving, enforces `capacity == target`, and returns early for zero-sized types. The minimum capacity of 4 is paid, so `substitute`'s over-capacity argument vectors are accounted for.
- **`compute`:** it checks `storage.first` after the builtin facts call, so a physical refusal isn't mistaken for a semantic `None`.
- **Fast paths:** both type-flags paths pay 0 first, so a sticky refusal is authenticated on leaf and cache-hit queries.
