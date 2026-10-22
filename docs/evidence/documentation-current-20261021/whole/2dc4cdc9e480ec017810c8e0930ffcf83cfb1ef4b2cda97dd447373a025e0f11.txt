# implementation-review review

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

**Scope:** I reviewed the pasted source by reading only. I did not compile or run anything. The `tests` module is declared (`receiving/typed.rs` ≈L56) but not pasted, so I cannot confirm any claim about tests. Line numbers are approximate, counted from the top of each pasted file.

## Blockers

I found no confirmed compile or semantic blocker in the pasted text. The apparent candidates all depend on code that is not shown.

## Concrete defects and gaps (non-blocking, but real)

1. **`entry` Arithmetic failure is not sticky** (`receiving/typed.rs` ≈L29-31).
   - `ok_or(Setup::Arithmetic)?` returns a `Failure::Setup(Arithmetic)`.
   - Unlike every other `Setup` failure from `reserve`, it never sets `self.first`.
   - A caller that swallows that error could continue and later `reserve` calls would succeed.
   - Practically unreachable, because the sizes are tiny. It still breaks the "first setup replay" invariant, so either set `self.first` or add a test that documents it.

2. **Unpaid frames for the typed helpers' own returns** (`reserve` ≈L6, `entry` ≈L28, `physical` ≈L42, `append` params ≈L20-24).
   - `reserve` and `entry` each return `Result<(), Failure>`. `Failure` embeds `Diagnostic` inline, so this carrier is large.
   - `physical` returns a bare `Failure`.
   - None of these frames is charged by anything, and `reserve` is invoked many times per field.
   - `append`'s `&mut Vec` and `&mut Receiver` arguments are also not charged beyond the `entry::<()>` carrier.
   - You flagged this as an open concern. From the pasted code alone it is a real gap against "every helper entry pays Result + `&mut`". It is only acceptable if the "prepayment covers it" argument is written down and tested.

3. **`append` exact-capacity check is not guaranteed by std** (`typed_fields.rs` ≈L37-41).
   - `if output.capacity() != capacity { return Err(owner.physical()) }` follows `try_reserve_exact`.
   - The std docs say the allocator may give more than requested, and `try_reserve_exact` does not promise exact capacity.
   - A larger capacity would be reported as a physical budget failure, although the charge (`layout.size()`) covers a smaller buffer than was actually allocated.
   - Use `<` plus a charge for the actual capacity, or document that exactness is assumed. I mark this as an uncertainty about the allocator, not a proven bug.

4. **Double-charging in `make`** (`typed_fields.rs` ≈L12-13).
   - `entry::<FieldInitializer>()` already charges `size_of::<Result<FieldInitializer, Failure>>()`, which includes the `FieldInitializer` itself.
   - The next line then reserves `size_of::<FieldInitializer>()` again.
   - This is consistent with the "cumulative, no discount" contract, so it is probably intended. Make sure the exact-width tests assert this double count, or it will look like a bug later.
   - `build` has a similar redundant `reserve(0)` straight after `entry` (≈L56). It is harmless.

5. **`text` sentinel detection relies on `reserve(0)`** (`receiving/typed.rs` ≈L39).
   - It assumes `reserve_active_required(0)` fails after `budgeted_clone` returned its empty sentinel.
   - That behavior lives in `bounded_output`, which is not shown. I cannot confirm that a legitimately empty `value` is distinguished from a refused one.
   - If `reserve_active_required(0)` does not consult a sticky refused flag, the sentinel would be accepted silently.

## Uncertainties (depend on unseen code)

- **`Setup: Copy`.**
  - `reserve` reads `self.first` by copy (≈L7).
  - It also uses `failure` after storing it (≈L21-22).
  - It compiles only if `Setup` is `Copy`. The same applies to `Debug` on `Failure`.

- **Legacy error conversions** (`typed.rs` ≈L52 and ≈L65).
  - `.map_err(Failure::from)` compiles only if the legacy error types are `Diagnostic` or `Vec<Diagnostic>`.
  - If any legacy path returns the old `super::Failure`, there is no `From` impl and the code will not compile. The two `Failure` types share a name, which makes this easy to miss.

- **`receive_typed` Setup arm** (≈L84-87).
  - It returns the passed `refusal` rather than `self.first`.
  - That is equivalent only because `reserve` and `physical` always return the stored first failure.
  - The one exception is the non-sticky Arithmetic case in item 1.
  - The semantic-first contract holds for `One` and `Many` only if `Receiver::receive` does not overwrite a stored first `Setup`. That function is not shown.

- **Foreign-owner poisoning.**
  - `reserve` calls `ledger.current()` and sets `detached`.
  - Whether it poisons only the original budget, and never the foreign one, depends on `ledger.current()`, which is not pasted.

- **Evaluation order in `make`** (≈L15-16).
  - `name` is allocated, then `default_expr` runs.
  - If the old code evaluated them in the opposite order, a different first failure would be selected under tight budgets.
  - I cannot verify this without the old helper.

## Things that look correct

- `reserve` replays a stored `first` before touching `ledger.current()` or ACTIVE, which matches "replay before touching foreign ACTIVE".
- `physical` authorizes via `reserve(0)` first, then issues `usize::MAX`, as the contract states.
- The closures borrowing `owner` in `ok_or_else` and `map_err` are sequential, so the borrow checker should accept them.
- `Some(core)` in one branch and `core` in the other are mutually exclusive moves.
- `append` grows only after the full field succeeds and before the push, preserving the old minimum-4 geometric growth and the 1024-byte element threshold.

## Recommended before merge

- Make Arithmetic sticky.
- Decide explicitly whether `Result<(), Failure>` and argument frames of `reserve`, `entry` and `physical` are covered, and add boundary tests for it.
- Relax the exact-capacity equality in `append`.
