# review (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Verdict: no concrete blocker found in the diff itself.** One risk lies outside the diff and is flagged below. I did not compile or run anything, so every point is source-read only.

## 1. Borrowed renewal adapters (`renewal_probes.rs:63, 70`)

- **Work charges:** The `.map(...)` is applied after `lookup(...)?`-free `Result` returns. All `units(work, …)` calls (lines 20 and 27) still run in full before `map.get`. The adapter adds no charge and skips none.
- **First error:** `Result::map` passes `Err(())` through unchanged. The first refused unit still short-circuits at lines 20/27. The error type is `()`, so there is no richer error payload to lose.
- **No cloning:** `rows` is `Option<&NominalPayload<T>>`. `&**values` dereferences `&NominalPayload<T>` to `NominalPayload<T>`, then through `Deref` (`nominal_payload.rs:194-202`) to `Vec<T>`, and re-borrows. Both `Storage::Owned(v)` and `Storage::Shared(v) => &v.values` return references to the existing storage, so values and lifetime are borrowed, not copied.
- **Lifetime:** The output lifetime is the elided `&self` lifetime. `&self.type_parameters` has that lifetime, and `lookup` returns `&'a V` with the same `'a`. The reborrow cannot outlive the payload reference, so it is exact.
- **Why the fix should typecheck:** In the 504b error, `V` was fixed by the return type to `Vec<…>`. With `.map(...)` following, `V` is no longer constrained by the return type. Deref coercion from `&IndexMap<P>` to `&BTreeMap<DeclarationId, ?V>` (`map.rs:21-25`, Target `BTreeMap<DeclarationId, V>`) can then infer `V = NominalPayload<T>`. I believe this works because rustc autoderefs the argument when the target contains an inference variable. I could not verify it without a compiler.
- **Uncertainty:** `declarations` (line 56) is unchanged and produced no 504b error. I assume it is a plain `BTreeMap<DeclarationId, Declaration>` and did not see its type.

## 2. Trait re-export (`elements.rs:3`)

- `ReplayClonePayload` is `replay::ClonePayload` re-exported at `cleanup_plan.rs:37`. That line is not in the diff and not `cfg(test)`-gated, and 504b raised no error on it, so it already resolved.
- The `use … as` rename imports the same trait item, so method resolution at line 66 uses the same trait whose `ResolvedType` impl is at `physical_clone.rs:168`.
- **Missing evidence:** `replay.rs` is not shown. I could not confirm that `replay::ClonePayload` is the `physical_clone.rs:7` trait rather than a different same-named trait.
  - If it were a different trait without a `ResolvedType` impl, the result would be a loud E0599 at line 66, not silent divergence.
  - The `ClonePayload` supertrait/impl on `ResolvedType` is the only one shown.
- The diff replaces a private path with a public re-export. This is a visibility-only change, with no behavior change.

## 3. `#[path]` module repairs (`prelude_bundle.rs:13-30, 154-159`)

- **Resolution base:** `prelude_bundle.rs` is itself loaded through `#[path = "hir/prelude_bundle.rs"]` (`hir.rs:18`). rustc therefore treats it as mod-rs-like, and its children resolve against `src/hir/`. This matches the 504b E0583 messages.
- **Path correctness:** The new `#[path = "prelude_bundle/X.rs"]` attributes resolve relative to that directory, giving `src/hir/prelude_bundle/X.rs`. That is the intended layout.
- **Not verified:**
  - I cannot see the filesystem, so I cannot confirm that all nine children plus `original.rs` and `tests.rs` exist there. A missing file would give E0583 again.
  - If any child declares its own `mod`, it would also be treated as mod-rs-like. Its grandchildren would resolve in `src/hir/prelude_bundle/`, not `prelude_bundle/<child>/`. The diff shows no such nested modules, but the child sources are not provided.
- The `pub(crate) use` reordering (lines 31-32) is cosmetic.

## 4. Risk outside the diff: `nominal_base.rs` errors

The 504b diagnostics include E0599 for `types_slice` (`nominal_base.rs:21, 44`) and `declarations_slice` (`nominal_base.rs:33`). The diff does not touch `nominal_base.rs`. The visible `BasePreludeBundle` has only `types()` and `declarations()` (lines 105-111).

- **Most likely explanation:** The methods are defined in an inherent `impl BasePreludeBundle` inside one of the child modules (probably `borrowed.rs`). Those modules failed to load in 504b, so the error is probably a cascade that the `#[path]` repair resolves.
- **Not established:** I haven't seen the children. If no child defines these methods, or defines them with visibility too narrow for `declaration_index`, these three errors remain and would be a compile blocker.

Treat this as the main item the live compile must confirm. It is an uncertainty, not a proven defect.

## Residual uncertainty

Because the E0583 errors hid the child modules, errors inside those modules (types, visibility, `use` paths) would only appear in a compile that loads them. The diff cannot show them. This review does not cover those children.
