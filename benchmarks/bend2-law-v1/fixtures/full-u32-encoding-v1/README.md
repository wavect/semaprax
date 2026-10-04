# Full-range u32 value encoding controls

This additive experiment preserves **every numeric value in 0..4294967295**.
It supplies executable no-op-transfer and empty-sort rejection controls on both
languages without requiring a new SEMAPRAX integer builtin. The original
`manifest.json` and its native checked-`u32` admission gates are unchanged.
This is a proposed representation profile, not a declaration that those gates
passed or that LAW-16 is complete.

The retained [report](../../evidence/full-u32-encoding-controls-v1/report.json)
records 12 candidate/attack outcomes across two tasks and three distinct routes,
plus four SEMAPRAX representation-boundary refusals and an actual Z3 numeric
representation check. The separate task routes are:

- Bend ordinary `--check-only`: candidate laws accepted, attack law rejected.
- Bend `--verdict`: candidate concrete equalities accepted, attack law rejected.
- SEMAPRAX `run --native`: candidate assertions return zero, attacks fail an
  unchanged postcondition at runtime. This combines native compilation and
  execution and supplies no source proof or timed comparison.

The Bend laws prove the listed concrete outputs by evaluation. They are **not
universal transfer or list theorems**. SEMAPRAX's main assertion compares exact
list outputs, including duplicates; the empty result fails it. Its transfer
postconditions require both exact changes and conservation, so a no-op fails.
Only the implementation body is mutated; laws and witness inputs remain fixed.

## Proposed representation contract

Profile: `semaprax.checked-u32-value-encoding.v1`.

The shared scalar domain is the entire closed interval `[0, 2^32-1]`. Bend uses
`U32`; SEMAPRAX stores the same mathematical value in `i64`. SEMAPRAX's public
benchmark entry guards refuse negative values and values above `2^32-1`.
The four physical boundary controls exercise these refusals for balances and
list elements. There is no fixed-small-number, sampled-input or signed-range
restriction in the entry's numeric domain.

Transfer accepts two account balances and an amount from that domain. It returns
a structured decision with two balances and a status:

1. If `amount > debit`, return status 1 and unchanged balances.
2. Otherwise, if `credit > MAX - amount`, return status 2 and unchanged balances.
3. Otherwise, return status 0, `debit - amount`, and `credit + amount`.

This is an explicit checked transfer API with failure values. It is not a claim
that Bend's primitive arithmetic traps, that SEMAPRAX now has native `u32`, or
that either primitive's failure ABI matches the other's. Both sides implement
the same failure order and preserve the original state on rejection.

The original `[9,4]`, amount `3` witness remains present. Additional witnesses
include zero-amount transfer between two MAX balances, transfer of the full MAX
amount, both underflow/overflow refusals, the case where both refusal conditions
hold, and crossing the signed-32 midpoint.

## Why the numeric representation preserves these operations

Let `M = 2^32-1`, and map each Bend word to its nonnegative integer value in
SEMAPRAX's `i64`. This map is a bijection onto `[0,M]`; comparisons retain their
order. At the pinned Bend commit, `bend2/base.bend` defines `U32.add` and
`U32.sub` through 32-bit `Word.add`/`Word.sub`; `Word.adc` drops the final carry
at its zero-width base case. These are wrapping primitive operations.

The transfer checks prevent any wrapping primitive from receiving an overflowing
operation: `M - amount` is always in `[0,M]`, the admitted debit subtraction is
nonnegative, and `credit <= M - amount` makes the addition at most `M`. Hence
Bend's results coincide with mathematical subtraction/addition and SEMAPRAX's
checked `i64` operations. SEMAPRAX's conservation assertion may add two balances;
its maximum intermediate is `2*M = 8589934590`, safely within `i64`. It never
compares that sum with a wrapping Bend `U32` total.

Sorting compares and copies elements without element arithmetic. The numeric
map preserves `<=`, equality, order, and multiplicity. The original
`[3,1,3,2] -> [1,2,3,3]` case remains, and a second duplicate-bearing case
includes MAX, zero, and the signed-32 midpoint. Empty and singleton inputs also
remain explicit. Exact output equality checks all multiplicities on each
witness; checking only length or sortedness would be insufficient.

The accompanying [`representation.smt2`](representation.smt2) also checks this
bridge with installed Z3 over **all 2^96 balance/amount triples**. It proves
matching guard decisions, full-width output values, conservation, ordering,
equality, and rejection of a positive-amount successful no-op. Two negative
controls produce satisfiable counterexamples when the overflow guard is removed
or the ordering is narrowed to signed i32. The exact raw result is
`unsat / sat / sat`. This is a checked mathematical bitvector encoding; it does
not authenticate the language-source translation or prove the actual sort
algorithm. A future universal theorem cell must bind the translated source and
full statement to its actual checker/kernel. The existing LAW-15
SEMAPRAX collection proof supplies an applicable comparison-only `List<i64>`
route; Bend's upstream Nat sorting proof must not be relabeled as a U32 proof.

Physical list resources remain separate: SEMAPRAX limits a spine to 8192 items
and bounds call depth; Bend and the host have their own resource limits. The
saved controls have at most four items. Resource failures cannot be counted as
proof rejection or benchmark wins, and this experiment establishes no full-list
runtime totality.

## Reproduce

Supply the exact tool paths and SEMAPRAX executable digest from the retained
report, or declare fresh explicit pins when producing a new observation:

```sh
python3 benchmarks/bend2-law-v1/full_u32_encoding_controls.py \
  --bend-root /absolute/path/to/pinned-bend \
  --bun /absolute/path/to/bun \
  --semaprax /absolute/path/to/semaprax \
  --z3 /absolute/path/to/z3 \
  --semaprax-sha256 sha256:EXACT_EXECUTABLE_DIGEST \
  --semaprax-build-commit FULL_SOURCE_COMMIT \
  --artifacts /absolute/path/to/new-control-artifacts
python3 -m unittest discover -s benchmarks/bend2-law-v1 \
  -p test_full_u32_encoding_controls.py -v
```

The runner refuses a dirty or wrong Bend `bend2` tree, a mismatched SEMAPRAX
executable, and preexisting artifact directories. It records executable digests,
version output, source bytes, raw stdout/stderr and individual results. Every
child receives `BEND_NO_TELEMETRY=1` and a 60-second timeout. Missing tools,
timeouts, parser errors and compiler failures never count as the expected attack
refusal. Normal Bend and verdict output markers are deliberately not
interchangeable.

The saved SEMAPRAX executable was reported as built at `c85b847a2`; its full
commit and SHA256 appear in the report. The script labels this source association
as caller-declared, not a build attestation. Its ordinary native route invokes
installed `clang` with C11, `-O2`, warnings as errors and the compiler's normal
flags; clang bytes and version are recorded. No Cargo command was needed to
collect these controls. Native failure scratch is retained by the compiler's
ordinary run command.

## Admission and remaining work

To use this route for existing tasks, add a **new versioned manifest** that
retains each original task/law/attack and input, explicitly declares the full
value encoding and failure API above, and keeps native-u32 admission as a
separate capability. Do not change an existing `u32 checked` cell into a small
bounded or Boolean cell, and do not treat this supplemental receipt as its
admission. The new profile must preserve the original report's nonresults.

Full closure still requires actual matched universal list/transfer assurance
where claimed, fixed-budget independent agent campaigns for admitted tasks,
measurements and cache/provenance work. Unsupported-cell receipts alone cannot
satisfy the explicit requirement that both languages reject the two loopholes.
The completed paired controls here remove the need to implement every compiler
numeric-width path merely to demonstrate those concrete rejections, while
keeping the broader benchmark work visible.
