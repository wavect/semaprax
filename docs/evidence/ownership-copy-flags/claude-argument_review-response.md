# claude argument review response (ownership-copy-flags)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Verdict: no definite API, privacy, type, syntax or parity errors in the supplied source.** Nothing was executed, so all of this is a source-only reading.

**Parity (old `_before` vs current)**
- After the facts lookup, the two bodies are textually identical. The only difference is `if facts.copy` in the old body and `if facts` in the new one, because `borrowed_ownership_copy` already returns `Option<bool>`.
- The `ok_or_else` diagnostic is unchanged: same `argument.span` and the same `"type `{}` has no semantic facts"` text with `identity_key()`.
- These parts are unchanged:
  - the `Own`, `Shared` and `Value` arms;
  - the `Borrow` branch order: RowLoan, reader, Bytes/vector, map collection, owned-bytes, runtime-admitted/direct variant, then `true`;
  - the place and projection checks;
  - the final error, which uses `param.identity` (the `ParameterIdentity` Display), the same span and the same message.
- `native`, `ownership`, `place`, `use`, `parameter ID`, `span` and `firstDiagnostic` are preserved. The only added error source is `Payer::authenticate()` reaching `reserve_active_required(0)`. It runs before the nominal lookup, so under a refused budget a nominal type errors where the old path would succeed. The third test asserts this, and I treat it as the intended sticky-zero behaviour, not a regression.

**Syntax, privacy and types**
- `#[cfg(test)] if … { return …; }` is a statement-position attribute, so it is valid. Moving `param` into the diverging branch is fine for the later use.
- `copy_before.rs` resolves as `call_parameters/copy_before.rs`, and `copy_flags_tests` resolves the same way.
- Glob-imported private items are visible to descendants: `ownership_flags`, `ParameterView::owned`, the private `identity` field, and the `pub(super)` `_before` method.
- `ownership_flags::Payer` is a unit struct, so it can be passed by value.
- The tuple destructuring in the tests matches the shape `with_limit_usage_refusal(measure(..))` returns, so `(((), x), refused, paid, first)` is correct.
- The `impl<'a>` and `impl<'_>` split on `borrowed_type_facts` and `borrowed_ownership_copy` is correct, since only the first returns a `Cow<'a>`.
- The `#[rustfmt::skip]` on the `_before` fn and the exact archive-containment test are consistent, assuming the archive text starts at `pub(super) fn validate_argument_ownership_view(`.

**Not verifiable from what was supplied (assumptions, not defects)**
- `Diagnostic: PartialEq + Debug`, needed for `assert_eq!(current, old)`.
- The facts type exposes a `copy` field, and `type_flags_paid` returns `Result<Option<_>>` with a `copy` field.
- `main.body.ownership == Value` and `main.body` being a `ResolvedExpr`, which the I64 test's `unwrap()` depends on.
- The `first.requested` field.
- Existence of `call_parameters/tests.rs`, `ownership_flags/{before,tests}.rs`, and the `docs/evidence/.../validate_argument_ownership_view.before.txt` archive.
- The file that holds the nominal helper tail, its `Cow` import, and that `borrowed_type_facts` is visible from `copy_before`.

**Coverage note (not an error):** `complete_argument_receiver_…` varies only the function body's kind. It sweeps argument ownership, parameter ownership and type, but it likely never builds a `RowLoan` or an exact-place argument. The Borrow branches are therefore protected by textual identity and the archive-containment test, not by behavioural comparison.
