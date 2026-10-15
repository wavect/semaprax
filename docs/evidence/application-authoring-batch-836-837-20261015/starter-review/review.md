# OPT837 independent source review

Reviewed checkpoint `604e7fb05203a6701be6bdbcda8a181c5b969df2` against `e3eb476838442b58c02ccc815aae4bbdfebfd50e`. Source-only: no build, tests, native execution, benchmark, paid call, or repository mutation. Context started from static Graft cards in the root checkout, followed by bounded changed files and relevant owner spans. This is not executable qualification.

## Finding: P2 — Bind the regular-file/non-symlink check to the opened input

Location: `crates/semaprax-toolchain/src/new_project/application.rs:19–30` (also the read at 33–35).

`read_bound` checks the path with `symlink_metadata`, then separately obtains a following `Handle::from_path` and opens the path with `File::open`. The checked metadata is never bound to the resulting file. Counterexample: after the metadata check succeeds on regular `plan.json`, replace it with a symlink to another valid plan before line 24. Both following handles refer to the symlink target, so their identity comparison succeeds. The final path identity check also follows the symlink and succeeds. This accepts an input explicitly forbidden by APPLICATION-STARTER-V1's regular non-symlink file contract. The same window exists for command activation. Replacing the path with a FIFO before line 24 can instead block `File::open` or the subsequent read; the byte limit does not bound that blocking.

Minimal fix: use the existing platform authority owner (or a safe facade over it) to open the selected regular input with final-component no-follow semantics, retain its identity, and read through that handle. Bind path rechecks to that held object, and validate the opened handle's file type. A post-open `symlink_metadata` check alone closes neither the possible FIFO blocking nor the initial open race. Preserve the final exact-byte recheck.

Missing regression: deterministic hook between initial inspection and open, replacing plan/command input with a symlink or FIFO; require refusal without staging or opening a blocking replacement. Include a normal stable regular input and initially supplied symlink control. Current application tests cover destination and parent symlinks, not selected input replacement.

## Focused missing verification

The new pure native-test selector has parser controls and a wrong-profile library assertion, plus one broad typed starter journey whose earlier source admission is explicitly known to fail. Add small direct profile-owner cases for exact-v31 pure typed helper admission, no-permit/function-effect refusal, and undeclared test root refusal; retain the old `--target native` v31 refusal. These should be separate from the large starter journey so an earlier codec/capacity refusal cannot prevent exercising the new boundary. No claim is made here that these controls fail; they remain unverified.

The reviewed publication branch reuses the held parent/stage publisher, create-new writes, exact inventory authentication, settled-file pivot, and uncertainty behavior. The new typed test branch explicitly calls the complete typed HIR validator with `None`, checks purity, and retains the original selector's profile restriction. No further concrete authority, overwrite, cleanup, receipt collision, or legacy widening defect was established within this bounded review.
