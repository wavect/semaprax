# Review of `6be3bb0` (pasted source only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Line numbers are approximate. I cite by function and expression because the pasted line ranges are partial and I did not count absolute lines. I found no definite compile error in the visible text. Several items depend on code that was not pasted.

## Probable blocker (depends on intended contract)

**1. An extension failure poisons base-only calls** (`ensure_selected`, the `if let Some(first) = self.first` early return and the `self.first = Some(first)` after `result`).
- The same `self.first` slot holds both base and extension failures.
- If the Vec extension hits `Bytes`, `Work`, or `Producer` on a `prelude.len() > 2` call, the invocation is poisoned.
- A later `prelude.len() == 2` call would then return that extension error, even though it needs no extension work.
- That contradicts "base2 unchanged" and "other extensions still run fully".
- If sticky whole-invocation failure is intended, document it and add a test. Otherwise, store the extension failure in a separate slot that is consulted only when `len > 2`.

## Uncertainties that need confirmation

**2. `VECTOR_WORK` is built from literals, not from the source it meters** (`extension_work.rs`, the `VECTOR_WORK` const).
- `"Vec".len()` and `"T".len()` are hardcoded, while `acquire` meters `source.name` and `parameter.name` from `declarations()[2]`.
- `VEC_ID.len()` is assumed to equal `source.stable_id`'s length, and `paid::identity` is assumed to charge exactly that.
- The assumed `paid::string` cost of `2*len` (copy plus UTF-8 pass) and `paid::vector(1, …)` costing 0 work are not visible.
- The "4" is three `work(1)` calls before the identity copy plus one before `push`. That matches the pasted code only if the `paid::*` helpers charge nothing beyond what is added here.
- A drift in any of these would make a successful `acquire` spuriously refuse for work, or leave slack.
- Needed test: on success, `receipt.used == receipt.limit`. Also assert that `declarations()[2].name == "Vec"` and that its parameter is `"T"`.

**3. Byte carrier in `acquire` may not match the real receiving type** (`vector_extension.rs`, the first `payer.bytes(...)`).
- It charges `size_of::<Result<DeclarationId, P::Error>>()`.
- The calls `paid::identity(...)?` and `paid::vector(...)?` sit in a function returning `Failure<P::Error>`. The `Failure<P::Error>` wrapper is charged for the vector but not for the identity.
- If `identity` actually returns `Result<_, Failure<P::Error>>`, the charge undercounts.
- The `Result` carrier for `from_funded_vec` and the `Vec::new().into()` target are not obviously charged either.
- This is an accounting hole only if those types are not covered by `2*size_of::<Self>()`.

**4. Receipt only carries work failures** (`ExtensionPayer::receipt`).
- When `first` is `Bytes`, `receipt.first` is `None`. `used` and `remaining` stay consistent, but a consumer cannot tell a clean partial run from a byte-refused one by the receipt alone.
- This is fine only if callers always consult `first_failure()`.
- The omitted error mapping (invocation lines 139–153) must prefer `first_failure()` over a re-derived `Failure`. Otherwise a `Failure::Payer(FixedFailure::Bytes)` could be remapped inconsistently.
- Test this explicitly for work refusal and for Bytes refusal at `reserve(0)` inside `work`.

**5. `reserve(0)` on every `work` call** (`ExtensionPayer::work`).
- Every unit of work invokes the byte owner. For a callback that counts invocations or is stateful, this is observable.
- It is intentional per the stated design, but it makes `work` able to select a `Bytes` failure before a `Work` refusal.
- Tests should pin the precedence: Bytes beats Work when both would fail on the same call, and a cached first failure never re-queries the callback.

**6. `FixedFailure` / `WorkRefusal` need `Copy`.**
- `*first` in `receipt` and the `Work(receipt)` copy in `first_failure` require `WorkRefusal: Copy`.
- The binding named `receipt` there is a `WorkRefusal`, not a `WorkReceipt`, which is misleading but harmless.
- `FixedFailure<E>` is hand-copied rather than `Copy`. If it derives `Copy` with `E: Copy`, the hand copy is redundant. If not, `InvocationFailure<E>` must still be `Copy` for `if let Some(first) = self.first`, and that depends on the derive for `FixedFailure<E>`.

**7. Borrow and lifetime shape** (`ensure_selected`).
- `&mut impl FnMut` is coerced to `&'a mut dyn FnMut` and kept alive in `payer` inside the closure.
- This compiles only if `reserve` is not used again while `payer` is live. Inside the visible code it is not.
- Any use in the omitted part before `payer` is dropped would be a borrow error. The `#[cfg(test)]` on an `if let` statement is acceptable as a statement, but `extension_limit` must also be `cfg(test)`-gated.

**8. `for_source` pointer-identity checks depend on `declarations()` returning a stable `&'static` slice.**
- If it builds a fresh `Vec` or slice per call, both `ptr::eq` checks would always fail, or `current[..len]` would not alias `prelude`.
- Also note the length set differs deliberately: `ensure_selected` accepts 2, `for_source` does not.
- A test should show that a len-2 prelude returns `None` from `for_source` and a copied-but-equal slice is rejected.

**9. `acquire` failure leaves `extension_receipt` unset** (`ExtensionPayer::acquire` error path in `ensure_selected`).
- Failure before a payer exists, at the initial `reserve(carriers)`, records no receipt and caches no failure in the payer. The outer `self.first` caches it.
- This is consistent, but the carrier count (`Self` plus `Result<Self,E>` plus `WorkReceipt`) does not include the `Result<ExtensionPayer, InvocationFailure>` the caller actually receives.
- If the `?` there maps into a larger type, the charge is short.

## Test gaps to add

- Restricted limits at 0, `VECTOR_WORK-1`, and `VECTOR_WORK`, each checking `used`, `remaining`, and `first`.
- Byte failure at each `bytes()` site, and at `reserve(0)` inside `work`.
- A callback that changes its error taxonomy after the first failure: the cached `E` must be returned unchanged.
- Repeated `ensure_selected` after failure: the callback must not be called again.
- A base-only call after an extension failure (see item 1).
