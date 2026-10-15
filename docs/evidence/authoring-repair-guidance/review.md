# review (authoring-repair-guidance)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Source-only review.** I did not run or build anything, so nothing here is a claim that tests pass. I found no blocker I can prove from the supplied text. A few points depend on definitions that weren't included.

## Verified from the supplied source

- **Primary diagnostic identity:** `with_authoring_repair_help` only sets `help`. It never touches `code`, `message`, `path` or `span`, and it early-returns when `help` is already set. The unit test strips `help` and compares to the original, which covers this.
- **Preserving budget, floor and sticky failure:** `reserve_active_preserving(len, 0)` takes `max(0, budget.floor)` and returns `false` without recording a refusal when `len + floor > remaining`. It then calls `reserve`, which returns `false` on an already-overflowed budget, so a sticky refusal rejects even fitting help. It never calls the `*_required` or `refuse_*` paths. The pre-check also makes the refusal branch inside `reserve` unreachable, so optional help cannot select a new refusal.
- **Exact String backing:** the debit is `text.len()`, `try_reserve_exact(text.len())` follows, and a capacity mismatch fails closed. The help `String` can be no larger than what was paid.
- **No invented names or authority:** the help text uses the placeholder `fresh_alias` and says to check it is unused. The T269 text only says to build bounded output and publish once, and it keeps the profile limits without naming a larger one. The match arms use exact message prefixes or equality, and a unit test covers the non-decorated cases.
- **Diagnostic type fit:** `Diagnostic.code` is `&'static str`, `message` is `Cow<'static, str>`, and `&Cow<str>` coerces to `&str`. `io_static` is `pub(crate)`, and `PartialEq`/`Eq` are derived under `cfg(test)`, which matches how the unit tests use them.

## Items to check against definitions I wasn't given

1. **Debit leaks when no help is attached.** After `reserve_active_preserving` has debited, a failed `try_reserve_exact` or a capacity mismatch returns the bare diagnostic. The budget stays spent and there is no refund API, so a failed allocation consumes budget while producing no help. This contradicts the comment "must never … consume the authoritative diagnostic floor". It is unlikely in practice but it is a real inconsistency. A cheap fix is to allocate and verify first, then reserve, and drop the help if the reserve fails. That ordering only works if allocating before paying is acceptable under your frontier policy.
2. **Early optional help versus later required work.** The floor passed is `0`, so protection depends entirely on `budget.floor` being set at that point in verification. If it isn't set, help on early diagnostics could use up budget that later required diagnostic allocations need. That would produce a sticky refusal that wouldn't happen without help. Please confirm the verifier sets the floor before `diagnostics::error` runs.
3. **Dead "existing help" guard on this path.** `error()` always builds a fresh diagnostic, so `help.is_some()` is never true at that call. Callers that later chain `.with_help(...)` will overwrite the optional help, and its debit stays paid for dropped bytes. If any M104 or T269 site adds its own help, that is a budget mismatch rather than a correctness bug.
4. **M104 and T269 emitters.** The prefixes and exact messages in `authoring_repair_help` must match what the verifier actually emits and route through `diagnostics::error`. I could not check this. The integration tests assume `"invalid or duplicate pattern binding `value`"` and `"stdout_write path reaches 2 sites; limit is 1"`.
5. **Test helpers.** `with_limit_usage`, `with_limit_usage_refusal` and `refusal.requested` weren't supplied, so I could not check their signatures. I also could not tell whether a limit of `0` means "no budget" or "zero budget". The `[0, length-1, length]` loop and the `used == 0` assertions depend on the latter.
6. **Integration-test `diagnostics(&corrected).is_empty()`.** This holds only if warnings are not included and removing the second `stdout_write` leaves an unused `let first`/`let value` without a diagnostic. I could not verify either.
7. **"`result` is reserved" claim.** The help text, the test and the quick-reference all state this as fact, and I could not confirm it from the supplied source.

## Docs and CLI scope

- The T269 row in `AGENT-QUICK-REFERENCE.md` and the `help.rs` test string match each other exactly. If the CLI table is derived from the doc, that is consistent. If it is a separate copy, it needs the same edit.
- The new M104 row isn't covered by any CLI test in this diff. If a test enumerates the codes in the table or enforces a row format or width, check that this row passes. It contains `:` and `/`.

## Optional improvements (not blockers)

- Mark `fresh_alias` as a placeholder in the quick-reference row, as the help text already does.
- Add a test that a failed `try_reserve_exact` or capacity mismatch leaves the primary diagnostic intact and does not consume budget. This depends on fixing item 1.
- Use a table-driven `&[(code, matcher, text)]` instead of the guarded match arms, so the unit test can iterate it directly.
