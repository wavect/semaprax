# Review of the cfg(test) codec808 frontier witness

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

I reviewed only the pasted text and ran nothing. The pasted source has no line numbers, so I cite quoted fragments.

## Blockers

1. **One witness cannot cover three receipt families.**
   - The seven owners show three distinct receipt families: 201/149, 408/7 and 200/79.
   - The witness runs one fixed profile, `NestedRequest{64,256}` then V2 max64/256.
   - If the owners share this exact fixture, the three families are unexplained. If they differ in policy or schema, the witness is not exact reproduction for most of them.
   - Pass condition: the `.unwrap()` payload (the SPX-G171 `Err` Debug) must match a raw receipt. That matches at most one family. The other families stay unattributed.

2. **The prefix has no iteration marker for the failing `.unwrap()`.**
   - The `for policy in [NestedRequest…, response]` loop runs `with_authenticated_project` twice, and each call runs derive and verify.
   - The `.unwrap()` panic does not say which iteration or step failed.
   - `COMPLETED` and `LAST_COMPLETED` are thread-local and pooled across every guard that settled in the prefix. Both traces are cleared only once, before `prepare_v2`.
   - You can recover the iteration without touching the prefix. After the panic, locate the leftover `native-codec-allocation-{pid}-*` dir and compare `src/schema.spx` bytes against the original and the first-iteration output.
   - Without that, attribution to the response iteration versus the Nested iteration is a guess.

3. **The trace shows totals, not the refusing site.**
   - `Trace` gives `initial`, `consumed`, the category sum, `prebound`, `layouts` and `mode`. It is settled once in `Guard::drop`.
   - Nothing visible records which debit was refused or in what order.
   - So this cannot attribute the "first frontier" or the inner operation. At best it gives a category profile of a settled attempt.
   - If the refused debit isn't committed, `sum == consumed` holds trivially and the trace says nothing about the refusal.

## Does max vs last suffice for the original refused call?

- **Max: no.**
  - It keeps the highest `consumed` over all settled guards in the prefix. That includes the earlier Nested iteration and earlier derive/verify cores.
  - Nothing ties it to the refused call.
  - A refused call may also consume less than an earlier successful one.
- **Last: only conditionally.** It identifies the refused attempt only if all of these hold:
  1. A `Guard` was started for the refused operation (the refusal is not before `start`).
  2. That guard settled, and no later guard dropped.
  3. The guard is the innermost refusing one. Guards nest through `previous`, and the outermost drops last, so `last` may be an outer core and not the inner one that refused.
  4. The refused attempt was the final retry.
- If the refusal happened before `start`, `last` is a prior successful operation. That would look like a refused-call trace but be wrong.
- Print `None`-ness and `owner`/`mode` explicitly, so absence of a trace is visible.

## Further risks

- **Drop assertions can change the observed failure.**
  - `Guard::drop` has `expect("core trace settles before its ledger")`, an `assert_eq!` on the owner, and an `assert_eq!(trace.sum(), trace.consumed)`.
  - On an ordinary `Err` return, a failing assertion would replace the SPX-G171 `.unwrap()` with a trace-assertion panic. The witness would then record a different failure.
  - If the core itself panics, an assertion in `drop` during unwinding aborts the process. `catch_unwind` cannot recover that and no trace is printed.
  - Check the payload text: it must be the SPX-G171 unwrap and not a trace assertion.
- **cfg(test) library vs integration-linked library.**
  - The failing owners are production-linked, without the guards. Presumably the existing native cfg(test) owners also fail.
  - Same-error behavior is evidence, not proof, of equal budgeting. Any cfg(test)-only code that affects budgets or validation breaks equivalence.
  - Do receipts match the raw SHA-verified ones? That is the only real equivalence check.
- **Thread-locals.**
  - If any part of the pipeline runs on another thread, the traces are blind to it.
  - If libtest reuses the thread (for example with `--test-threads=1`), clearing both takes at the start handles stale state.
  - `CURRENT` is not cleared, but guards restore it.

## Fixture-fidelity cautions

- **Source-equality control.**
  - A compile-time comparison proves the prefix text and manifest bytes are identical. It does not prove identical semantics.
  - `SCHEMA`, `canonical`, `SERIAL`, `project`, `fs` and `Ordering` resolve by module scope. A copy has its own `SERIAL`, which is harmless because pid and nanos still distinguish roots. Different imports or a different `canonical` would not be.
  - Compare against the archived helper's `SCHEMA` and `canonical` bytes, not only the prefix.
- **`include_str!` path.**
  - `"../bulk_utf8/semaprax.toml"` resolves relative to the containing file.
  - A new file at `workspace_graph/tests/codec808_frontier.rs` resolves differently if the original sits at another depth.
  - Under a crate-root `tests/`, the file would be an integration test, not a lib unit child, which contradicts the premise. Confirm the module is declared under `#[cfg(test)] mod`.
  - A mismatch is a compile error, not silent, but verify the intended resolved path.
- **Mechanics.**
  - `catch_unwind` needs `AssertUnwindSafe`.
  - The `response` profile must be `Copy`; the prefix already uses it twice, so it appears to be.
  - Print after capture. `resume_unwind` with the original payload is fine.
  - Cleaning the test dir only on success is correct, and the failure leaves the dir for the schema comparison above.

## Uncertainties, not blockers

- The meaning of receipts like 201/149 is unstated. Do not read allocation into them.
- It is unknown whether the refusing core is nested or a single resolution.
- Success on this witness means "prefix passes in the cfg(test) library". It says nothing about integration failures, as the proposal itself notes.

## Verdict

- This is a reasonable frontier-bounding witness.
- It is not by itself sound first-frontier attribution for the three receipt families.
- Gate the claims on three checks:
  - an exact match between the payload and a raw receipt;
  - leftover schema bytes identifying the iteration;
  - presence and owner of `last`.
- Treat `max` as context only.
