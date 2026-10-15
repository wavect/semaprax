# claude final (capacity-census-811-residual)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Source-only review of OPT811.** I ran nothing and have not seen `legacy`, `ordinary_view`, `byte_writer` or `lexical_control`. I found no type or signature errors in the new API: closure shapes, `Cell::get` paths, `bool::then`, the `?` inside `try_fold`, and `Copy` use of `Option<Components>` all look sound. The substantive findings follow, most important first.

**1. Retained empty-index builder: `ordinary_index` is attached too late (`helper_index.rs`, `with_index` / `with_helper_sites`).**
- `with_helper_sites` receives `ordinary` but the struct literal sets `ordinary_index: None`. `with_index` assigns it afterwards, and only then calls `index.ordinary_complete()?`.
- If `seed_entry` consults `renewal_binding` or notes capacity, it takes the legacy per-function path instead of the index.
- In the empty-index case (`Owned(empty)` plus a held ordinary directory), `helper_backing` adds the directory only when `ordinary_index` is `Some`. A seed-time sample would therefore omit the index, which breaks the "builder samples already include the held index" premise that phase pairing relies on.
- The completeness check also runs after the index has been used.
- Fix: put `ordinary_index: ordinary` in the literal, and call `ordinary_complete()` before constructing.
- Separately, `helper_backing` uses `matches!(Owned)` as a proxy for "ordinary not already counted". An exhaustive match on `(byte_helpers, ordinary_index)` would make the no-double-count invariant explicit.

**2. Same-phase lifetime and pairing.**
- `phase_overlap_upper <= component_maxima_upper` is not an invariant. A held-only sample (`held + staged + attached`) can exceed `builder + staged_max + attached` when no builder has noted, such as a zero-function program or a refusal before any builder runs.
- The test asserts it for one hand-built case, which is fine, but don't document or assert it generally.
- Nothing samples the window where a builder is still live and its finished plan exists but is not yet pushed. `staged` only sees the plan after `push`. If builder notes don't include the finished plan, the phase figure undercounts there.
- `Components.held_index` is never cleared when the index drops. This is harmless today because the index drops after the last sample, but it is order-dependent.

**3. Overflow-invalid stickiness.**
- The sticky flag itself is sound: `sample_phase` and `staged` mark invalid, `Drop` re-checks the sum, `OBSERVATIONS_VALID` stays false until reset, and the final block of the phase test covers it.
- `conditional_backing` and `string_owner_moves` use unchecked `*`, `+` and `sum`, which panics in debug and wraps in release. `helper_backing` saturates silently. Neither path can mark the census invalid.
- `expression_backing` returns 0 and drops the invalid mark when called outside a `Construction`, because `update` is a no-op there.
- `lower_capacity_high_water()` still returns a bare `usize`, possibly saturated, while the other two getters return `Option`. That is compatible, but undocumented as ignoring validity.
- `#[derive(Default)]` gives `valid: false`. Every call site correctly overrides it, but it is a trap. Prefer a manual `Default` or an inverted `invalid` flag.
- `Construction::drop` uses `expect`, which will abort if a drop happens during unwinding or out of LIFO order. Consider tolerating `None`.
- The final block of the phase test depends on the sticky state left by the previous block with no reset. That is intentional, but a comment would help.

**4. Compatibility with the old pin and assertions.**
- `lower_capacity_high_water = STANDALONE.max(OVERLAP)` is `>=` the old value and excludes the phase figure, so the old pin holds.
- Notes inside a `Construction` no longer touch `STANDALONE` but do reach `OVERLAP` via the sum.
- Naming collides: `standalone_builder_high_water()` reads `BUILDER_MAX`, which includes in-construction builders, while the `STANDALONE` static is outside-only.
- The `*_upper` names for the phase and maxima getters overclaim given the no-bound stance. Discrete sampling can miss peaks, so `_census` or `_sampled` is more honest.
- `observe` in `tests.rs` reads `LAST` after the oracle `build_plan_for_execution` loop. If the oracle ever opens a `Construction`, `LAST` is clobbered. Capture it right after `build_program_plans`, as `observe_current_only` does.
- `observe` asserts `upper() == builder + staged + attached`, but `upper()` uses `staged_max`. They are equal only because staged bytes grow monotonically during a build. Use `staged_max`.
- `observe` uses plain `+`, which can overflow-panic; use `checked_add` or compare against a saturating sum.

**5. Vacuous retention tests.**
- `staged_component_keeps_earlier_retained_maximum…` and the retention check in the phase test call `plans.clear()`, which keeps `capacity`. If `CleanupPlan::unresolved()` owns no heap, `full == after-clear` trivially and nothing is proven.
- Use a plan with nonzero owned capacity, or replace the Vec with a smaller one before the second `staged` call.
- `checked_owned_variant…` asserts `delta == conditional + string_owner_moves`. That equality also encodes whether the legacy census already counted `HelperBindings`. The empty-index test shows the legacy side counts nothing for helper sites, so a non-empty owned helper fixture would break it. Check that the fixture has no byte helpers, or add that term.
- `staged()` rescans every plan on each push, which is O(n²) over the `LARGE` fixture. That is test-only but may be slow.

**6. No-claim hygiene.** The module header and test comments consistently avoid allocator-equality claims. `string_owner_moves` charging a `BTreeSet` header per element is clearly a legacy allowance, which the header already covers.
