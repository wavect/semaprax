# Review of the nine-slot extension producer (source only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Blockers

**B1. Unpaid `DeclarationId` clones (`extension_producer.rs`, `field_type`).**
- The type-parameter branch does `owner: owner.clone()` after only `work` has been paid, with no `bytes`.
- The nominal branch does `prior[target]...id.clone()` after the scan, and again pays nothing.
- Your own `identity` helper pays "exact backing" for IDs, so `DeclarationId` appears to own heap memory. If so, both clones allocate outside the byte accounting.
- The nominal clone also happens before the recursive `field_type` call. If that call is refused, a payer refusal is followed by an unpaid allocation and a drop.
- This stays a blocker unless `DeclarationId` is `Copy`, `Rc`, `&'static`, or otherwise allocation-free to clone. In that case, say so in a comment.
- Fix: pay `id.len()`-style backing before each clone (and the work if your model charges it), or carry a paid ID handle.

**B2. Likely type mismatch or unpaid conversion for `fields` and `arguments`.**
- The record branch pays `NominalPayload<ResolvedFieldDeclaration>` bytes and builds `Vec::new().into()`. That implies `Record.fields` is a `NominalPayload`.
- The variant path stores `fields: fields.take()`, which is a raw `Vec<ResolvedFieldDeclaration>`. It never calls `from_funded_vec` or pays a `Result<NominalPayload<…>>` carrier.
- If `ResolvedVariantCaseDeclaration.fields` is a `NominalPayload`, this does not compile.
- If a `From<Vec>` exists, the conversion is not shown to be paid or to be iterative.
- The same question applies to `ResolvedType::Nominal { arguments }`. It receives the raw `vector(1, …)` result, while `dispose` uses `.len()` and `.pop()`.
- If either field is `NominalPayload`, the code is wrong. If both are `Vec`, the record branch's `NominalPayload` payment is misleading.
- This needs resolving against the real AST definitions. I can't tell from the paste which case holds.

## Likely defects and uncertainties

**U1. Return and carrier accounting is inconsistent.**
- `acquire` pays `Result<ResolvedTypeDeclaration, Failure<_>>`.
- `field_type` returns `Result<StagedType, Failure<_>>` and `fields` returns `Result<Fields, Failure<_>>`, and neither pays that carrier.
- If your model counts stack carriers, those are unpaid.
- `field_type` pays `StagedType` and `Option<ResolvedType>` at each of its two frames. `fields` pays `Fields` and `Option<ResolvedType>`. This looks like deliberate over-charging, but the reason isn't documented.
- Per-element `bytes::<ResolvedFieldDeclaration>`, `…CaseDeclaration` and `…ParameterDeclaration` are paid in addition to the exact `vector` backing. That may double-count.
- Over-charging is harmless only if the caller's prepay and output bound use the same formula. The caller isn't visible, so I can't confirm this.

**U2. Slot-to-declaration mapping is unchecked.**
- `declarations().get(slot + 2)` assumes a fixed offset and an order identical to `NAMES`.
- `WORK` is computed from `NAMES` lengths and the `*_ID` constants. Nothing verifies that `source.name` and `source.stable_id` match them.
- `acquire` accepts any `Variant` or empty `Record` for any slot. It never checks that the kind matches the slot (variants only at slots 3 and 5).
- This is only safe if the "authenticated exact static AST prefix" is checked elsewhere.

**U3. Const requirements.**
- `STEP_ID`, `DONE_ID` and similar must be `&str` consts for the `const fn` calls to compile.
- `NAMES[i]` indexing in `const fn` is fine.

## WORK algebra (checked by reading, not running)

- **`record`:** matches `acquire`'s three `work(1)` calls, then `id.len()`, then `2·name`, then `1 + 2·param` per parameter.
- **`case` and `field`:** match the per-case and per-field `work(1)` plus `identity` plus `text`.
- **`nominal_argument`:**
  - The outer `work(1)` plus the scan `Σ(NAMES[i] + NAMES[target])` through the match gives the first terms.
  - The inner `work(1)` plus one parameter compare of `1 + 1` gives `+3`.
  - The total is scan + 4, which matches the code.
  - Hand check for `IterStep.rest`: 7 + 7 + 8 + 4 = 26, which matches.
- **Unmodelled work:** the bound is exact only if `vector`, `identity`, `text` and `from_funded_vec` do exactly the work you described. `from_funded_vec` is the one most likely to be wrong, since it is described as paying "control/owner/iterative scratch", which sounds like bytes, not work.
- **Early exits:** the bound is an upper bound on early exits. It is not exact on failure paths, which is fine if the cap is per-slot.

## Drop and recursion

- **Staging types:** `Type`, `Fields` and `Cases` drops are iterative via `dispose`. Their frontier is at most one argument, and `Option` is used as the pending slot, so they allocate nothing.
- **Panics in `Drop`:** `dispose` has an `assert!` and an `unreachable!`. These are not recursion or allocation issues, but a panic during unwind aborts.
- **Finished declarations:** these drop with derived glue. Their nesting is at most `Nominal → arguments → TypeParameter`, so the depth is bounded by the fixed AST. That is acceptable only because of that finiteness.
- **Shadowed `cases`:** the original `Cases` lives to the end of the block and drops after `from_funded_vec`. This is fine if `from_funded_vec` leaves `cases.0` emptied on success and intact on `E`, which you state it does.
- **Ordering in `field_type`:** `vector(1)` is fallible, and `value` is a guarded `StagedType` at that point, so a refusal disposes it. `arguments.push(value.take())` and construction of the root happen with no callback in between. That ordering is correct.
- **`Fields::take`:** it uses `mem::take`, so the subsequent `Drop` sees an empty vec. Correct.

## Tests

- No tests were pasted, so nothing here is covered.
- Tests worth adding:
  - Per-slot exact `WORK` equality against a counting `Payer`.
  - A refusal injected at every `bytes` and `work` call, with a counting allocator asserting no allocation after refusal and no leak.
  - Sticky first-error behaviour.
  - A `NAMES`/`declarations()[slot + 2]` identity-and-kind check for all nine slots.
  - A compile-time assertion that the `fields` and `arguments` field types match what the producer builds.
