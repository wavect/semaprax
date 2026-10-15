# Review of b2be954: findings

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only review. Nothing was executed, and `callee_components`, `Subject`, `TypeTable::declaration`, and `Recovery::{reads_value, view}` were not in the paste.

## F1: Known built-in outer carriers may lose T224 (severity: potential blocker, uncertain)

- **Where:** `signature_refusal.rs:20-25` and `:35-41`; callers at `functions.rs:360-366` and `:384-390`; same gate at `signature_refusal.rs:105-112`.
- **Problem:** The "unresolved root" test is only "not a type parameter and `types.declaration(name)` is `None`". If `declaration` has no entry for built-in generic carriers (`Option`, `Vec`, `Result`, `Box`), then `Option<Missing>` passes that test. The scratch check then reports T220 for the child `Missing`. The message suffix at `:40` matches, so `unknown_component` returns true. That suppresses the independent known-outer T224, which is exactly what the header comment at `:17-19` forbids.
- **Why it matters:** The suffix check at `:40` and `:141` never ties the diagnostic to the root name or to the diagnostic's own slot. Any in-scope-parameter T220 from any nested position counts.
- **Evidence gap:** The only outer-known test (`tests.rs:540-546`) uses a user record, which has a declaration. Nothing covers a built-in outer, so this is unverified either way.
- **Resolution:** Confirm that `declaration()` covers built-ins. If it does not, bind the refusal to the root identity, for example by requiring the diagnostic message to name `name`.

## F2: Return recovery is gated twice with different predicates (severity: medium, uncertain)

- **Where:** `recovery.rs:131-136` against `signature_refusal.rs:113-116` and `:134-143`; the suppression at `functions.rs:1086-1088`.
- **Problem:** `signature_origin` returns an empty `Recovery` early unless some diagnostic satisfies `callee_components::unknown_component_receipt`. `return_refusal` independently accepts `SPX-T001` or T220-with-suffix. If `unknown_component_receipt` is narrower, such as T001 only or a different T220 form, two things happen:
  - `return_unknown` still suppresses T103 at `functions.rs:1225`.
  - The `result` binding at `recovery.rs:190-198` is never installed, because the function returned early.
- **Consequence:** `ensures result` keeps a dependent C101 while T103 disappears, so the outcome is only partly deduplicated.
- **Evidence:** The T220 case at `tests.rs:571` uses `Missing<T>`, so it suggests the receipt does accept this T220. Whether it accepts the suffix form is not visible.

## F3: Return authentication differs from the documented six-field claim (severity: low, uncertain)

- **Where:** `signature_refusal.rs:143` compares with whole-`Diagnostic` `==`. `recovery.rs:100-107` and `:172-179` compare exactly six fields.
- **Problem:** The docs (`:147-148`, `:161-162`) describe both paths as full or six-field equality interchangeably. If `Diagnostic` has fields beyond those six, the return path is stricter. That fails closed, so it is safe, but the behaviours differ.
- **Test gap:** `tests.rs:572-591` mutates only the six fields and so does not exercise any extra field.
- **Resolution:** Either use a shared six-field comparator, or document the difference.

## F4: The foreign-subject test may be vacuous or wrong (severity: low, uncertain, test only)

- **Where:** `tests.rs:592-597`.
- **Problem:** `program.clone()` is used as the "foreign" program. The outcome depends on how `Subject::same` defines identity:
  - If it is address-based, the clone is foreign and the assertion holds.
  - If it is content-hash or path-based, or shares an `Arc`, the clone is the same subject, and `.is_none()` at `:597` fails or tests nothing.
- **Resolution:** Use an independently parsed program with a different identity.

## F5: Lifetime unification at the new call sites is unproven (severity: low, uncertain)

- **Where:** `functions.rs:1200`, `:1216`, and `:1298`; signature at `diagnostics.rs:190-201`.
- **Problem:** `function` comes from the local `Cow` (`functions.rs:1042`, `:1085`). The call requires a single `'p` for `&'p dyn ProgramSource`, `&'p Function`, `&'p TypeTable<'p>`, `&'p HashMap<&'p str, &'p Function>`, and `SignatureOrigin<'p>`. That compiles only if `TypeTable<'p>` is covariant in `'p`. If it contains interior mutability, which would make it invariant, then `types: &TypeTable<'p>` could not shrink to the loop-local lifetime. The shadowing at `:1086` and the origin construction at `:1087` also create short borrows whose lifetimes are not visible here.
- **Evidence gap:** I cannot decide this from the paste.

## Checked, with no defect found

- **Foreign subject and stale body:** `template_for` (`signature_refusal.rs:151`) fails closed on both. When the origin is `None` or mismatched, `signature_origin` returns `None`, and `iterative.rs:399-411` keeps the default recovery.
- **Current binding equality:** Each binding is matched against the actual `ty` and `mode` (`recovery.rs:141-144`), while the raw recheck uses the authored template's type, span, and parameter environment (`:145-167`). The `origin()` guard at `signature_refusal.rs:78-85` covers name, mode, and span.
- **`result` carrier identity:** The pointer check at `recovery.rs:190-191` ties it to `current.return_type`. The `enter.rs:134-139` guard requires `result_type.is_some()`, so preconditions and bodies gain no `result` binding.
- **T224, T202, and C101 for independent errors:** The changes only add `&&` suppressors and the `return_unknown` OR, so primary T220 and independent T224/T202/C101 stay unless F1 or F2 applies.
- **Storage:** No new funded storage claim is made. The error-only scratch vectors, the `origin()` scans, and the repeated `position` searches are all ordinary unpaid allocations.
