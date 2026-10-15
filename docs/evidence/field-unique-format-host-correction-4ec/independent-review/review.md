# review (independent-review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Verdict

I found no concrete blocker in the diff. I did not run anything, so everything below comes from reading the pasted source. Three items are uncertainties or coverage gaps, not proven defects.

## What I checked and found consistent

- **Operation semantics (host.js 323-354 vs browser_runtime.js 205-231).** The branches match:
  - Op 0 requires a zero right operand and copies without UTF-8 validation.
  - Op 1 validates both operands as UTF-8, applies the `a > 65536 - b` overflow guard, then concatenates.
  - Ops 2 and 3 render the signed and `asUintN(64)` decimal forms, and both require a zero right operand.
  - Any other operation is an invariant failure.
  - Result size is capped at 65536, and a `RangeError` maps to `0n`.
- **UTF-8 (323-331).** The host `validUtf8` (123-141) rejects overlongs, surrogates, values above U+10FFFF and truncated sequences. The `T{}` corpus, with an ASCII literal plus decimal digits, passes it.
- **Failure ownership.**
  - Every refusal path returns `0n` before any allocation.
  - `ownBytes` is the only allocating call and runs last (349), so a refusal consumes nothing.
  - Invariant violations throw plain `Error`s, not `status:`, so they fail the test at line 915 and are not misread as selected statuses.
  - Operand carriers are only read, never dropped, which matches doc lines 34-39 (the generated worker releases its own inputs).
- **Accounting.** `ownBytes` increments `copies`, and the worker's scratch drops go through `spx_bytes_drop`, so the `copies !== drops` and `payloads.size` checks at 926 stay meaningful for the format result.
- **Re-entrancy.** `allocationOutsideSort('literal format')` is called from `main`, outside sort, field-read and uniqueness, so the guard won't misfire.
- **Missing-import check (770-776).** It runs before the mode branches and removes only the format import. It expects `LinkError`, which is the correct failure for a missing function import, and it does not run guest code. It asserts the same property as the existing clone, sort and unique checks.

## Uncertainties and coverage gaps (non-blocking)

1. **Live-entry cap divergence from the contract.**
   - The browser returns `0n` when `entries.size >= maxLiveEntries` (browser_runtime.js 225). Doc lines 57-61 call a smaller host limit a checked policy that yields `semaprax.string-format.v1/1`.
   - The host's `formatStep` has no such check; it only tests `nextPayload > 0x7fffffff` (348).
   - In `copyRecordCodec` mode, `ownBytes` throws a plain `Error('Copy codec owned entry capacity')` at `payloads.size >= 16` (107). The `catch` at 350-353 rethrows it, so a cap would surface as a fixture crash, not a checked refusal.
   - Whether any `copy-record-codec` source uses `string_format` is not visible. `COPY_MAIN` does not, so this is probably unreachable today.
   - It is fine for the default Vec mode if the comment's "existing collection policy" means no live-entry cap. The 4096 live titles plus scratch would exceed the browser's 64, which is consistent with the stated distinction from the standalone browser table. I can't confirm the policy beyond that comment.

2. **Shared-owner mode.** In `shared-owner` modes, `ownBytes` allocates through `shared.payload.own` (106), so `nextPayload` (348) is never advanced and the token-exhaustion guard is vacuous. Real token or entry limits would belong to the shared host, which I can't see. Behavior on refusal there, whether `0n` or a throw, is unverified. The 4096-row corpus runs in `none` mode, so this does not affect the reported failure.

3. **The failure/UTF-8/cap behavior is untested.**
   - The only exercise of `formatStep` is the happy path: ASCII `T`, small decimals, and op 0 on literals. No control checks:
     - invalid UTF-8 in op 1,
     - the 65536 overflow returning `0n`,
     - a nonzero right operand,
     - an unknown operation, or
     - the `0n` refusal reaching status 1 with settled owners.
   - The linkability check proves only that the import is required. "Same operations as browser" therefore rests on code reading, not on a differential check.
   - A small `unique-controls`-style block calling `env.spx_format_step_v1` directly would close this. It could use invalid UTF-8 and an oversize concatenation, and assert `0n` with unchanged `copies`, `drops` and `nextPayload`.
   - This is optional for fixing the reported import failure, but it matters if the commit message claims behavior parity.

## Minor observations

- The `RangeError` to `0n` mapping inside `try` could mask an unrelated `RangeError` from `read` or `view`, since both can raise one via typed-array or DataView constructors after the manual bounds check. This is identical to the browser and unlikely to be reachable.
- Op 0 and the numeric ops do not range-check `left` as i64, the same as the browser. The Wasm ABI guarantees i64, so this is not a defect.
- The test comment at field_unique.rs 78-80 mentions a "32-allocation envelope". I can't tie it to the host's behavior from the pasted source, so I treat it as unverified.

## Not reviewed

The generated worker, `string_runtime::program_uses_format`, and C and interpreter parity were not in the paste. Claims about status 1 mapping and the worker's drops come from the doc text, not from code.
