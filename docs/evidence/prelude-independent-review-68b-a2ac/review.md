# Review of prelude forecast correction (source-only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Verdict
I found no concrete arithmetic, double-count, or Rust-typing defect in the pasted lines. The matches below are read from source, not run. Several conclusions depend on code that was not pasted, and I list those as uncertainties. The correction is source-consistent but unqualified, as `docs/WORKSPACE-GRAPH-PRELUDE-FORECAST-V1.md` itself says.

## Source-proved (with limits)

1. **Formula matches the existing declaration cost.**
   - `ast_type_declaration_cost` (expected_projection.rs:430-480) with `StructuralCost::new()` has `embedded_headers == false`, so `embedded_string` falls through to `string`. Each string costs a `size_of::<String>()` header plus its length in `total`, and its length alone in `string_bytes` (cost.rs, `string`).
   - `input_cost` (prelude_forecast.rs:15-38) charges the same terms:
     - `size_of::<TypeDeclaration>` plus id and name strings.
     - `size_of::<TypeParameterDeclaration>` plus its name.
     - `size_of::<VariantCaseDeclaration>` plus id and name.
     - `size_of::<FieldDeclaration>` plus id and name, then `size_of::<Type>` plus the parameter-name string.
   - `total - S` therefore keeps string headers in the structural term and the content bytes in the ×64 term. That is the same split as the main forecaster (`structure_input = hir_input - string_input`).
   - Caveat: `ast_field_cost` is not pasted. Parity for the field terms depends on it being `value(field)`, two embedded strings, and `ast_type_cost`. The doc claims a zero-allocation parity test checks this, but its source is not shown.

2. **Identity slots.** `ast_type_declaration_identity_slots` (identity_slots.rs:28-50) counts one slot per `Type::Named` with no arguments. `input_cost` adds `slots += 1` per field. Declaration, case and field stable IDs get no slot under the existing rule, and the new code does not invent one. Only their lengths enter `M`.

3. **Single insertion.** In held_projection.rs:~262-270, `prelude_hir_upper` is added once into `hir_upper`, after the mode-dependent `fixed_hir_upper` (mode 7 `cached_exact::fixed_upper` or the `fixed_hir_discount` branch). It therefore cannot consume an expression discount and is independent of layout mode 0..7 and of the parsed/semantic retention profile.
   - `prelude_forecast_original.rs` is identical except for this term, so the expected delta is `P` in `retained_hir`, `retained_clone_and_hir` and `raw_clone_and_hir`.
   - `synthetic_ast`, `runtime` and `transient_import_clone` are untouched.
   - `raw` and `runtime` never traverse base declarations (the census walks `program.types` only), so the shown code adds no second charge for the same declarations.

4. **Typing.**
   - `&[&Variant]` accepts `&BASE`.
   - `&&'static str` deref-coerces to `&str` in `cost.string(parameter)` and `parameter(name)`.
   - The nested `flat_map` closures return iterators over `'static` slices, so they should not borrow from the closure.
   - `str::len` over `&str` and `fold(…, usize::max)` are well-typed.
   - `checked_usage(0, bytes, …)` is used consistently with other call sites.
   - These are reading-level checks, not compilation.

5. **Constructor equivalence.** `base_schema::declaration` and `base_original` agree field by field on:
   - the ID constants and the case and field ordering;
   - `explicit_id: true` and the default spans;
   - `extends` and `invariants` being `None`.
   - Slice-`map().collect()` is exact-sized, so capacities should match `vec![]`. The physical-Drop equality is plausible but depends on the unseen observer.

## Items to resolve before relying on the correction

**Blockers for merge or qualification (acknowledged by the doc, not defects in the math):**
- **B1. Numeric receipt pins.** Every forecast now rises by `P` for all programs, so existing numeric pins and fit/refuse boundaries are stale by construction. The doc says they are deliberately not regenerated. Any test asserting old retained totals or fit/refuse at the cap will fail at this commit. Until an authorized run regenerates the pins, the tree should not be treated as green or as having a passing receipt.
- **B2. Gates not run.** No exact, one-short or zero boundary test, old-forecaster delta test, or carrier highwater gate is pasted or executed. Nothing in this source establishes them as passing.

**Uncertainties:**
- **U1. Conservativeness of the factors.** "P bounds the real retention" rests on factors 24, 64 and 16 being valid for a type declaration's index and resolved-HIR copies. The compile-time assertions cited as support are not shown, and the doc itself says the bound is not an allocator measurement. So the shown source only supports "P matches the existing factor model."
- **U2. Per-call versus per-program charging.** `P` is added per forecaster invocation. If a workspace sums per-module forecasts, or the resolver retains the prelude once per workspace, then `P` is either overcounted or correct only per call. That is conservative but not exact, and the caller aggregation is not shown. The doc's "once" claim is about one invocation.
- **U3. Authored overlap.** If authored `program.types` or an imported module also contains `Option` or `Result`, as a core library module could, both that source and `P` are charged. This is a conservative double count by design, and the doc's overlap test is parser-only.
- **U4. Other consumers of `hir_upper`.** Only this producer is shown. Any other site that derives a bound from `fixed_hir_upper` alone, or recomputes `retained_hir` (cached_exact, replay or resolver gates), is unchecked.
- **U5. Missing observation field.** The `forecast_observation::record` array (17 entries) does not record `prelude_hir_upper`. `retained_clone_and_hir` includes it, but a test cannot isolate `P` from the observation alone and would need to recompute it.
- **U6. Test infrastructure.** The base_schema test assumes the following, none of which is shown:
  - `measure` returns a `PartialEq + Debug` value with `invalid`, `live`, `allocated` and `freed` fields.
  - `measure` is isolated from concurrent test threads. If the observer is process-global and not serialized, `current == saved` could be flaky.
  - `base_original` is declared in `prelude` under `cfg(test)` or equivalent visibility.
- **U7. Extended prefix.** `declarations_for_program` selects up to 11 declarations when Vec, Box, iterator, list, stream or map are used. Those remain under the old accounting, so a program using them can still be under-forecast. The doc disclaims sufficiency for them, and it is not a regression.
- **U8. Redundant maximum fold.** `maximum_identity` passed in is already the maximum over `all_ids`, so folding base IDs again is harmless. It only matters when the caller passes `Some(x)` without base IDs, which the fold handles correctly.

## Contract statement
The document's supersession scope is narrow and consistent with the code: only the unconditional Option/Result term changes, with no cap, factor or public-schema change. Historical receipts are prior-model evidence, and current ones are unqualified until the gates run. Nothing pasted contradicts that. Nothing pasted proves allocation savings, causality or sufficiency, and none is claimed.
