# Task equivalence: `iterative-repair-workflow-v1`

This held-out task is this suite's content-level answer to issue #211's
"Agent workflow" category (see `docs/METHODOLOGY.md`'s "Taxonomy mapping"
section, and its now-closed "Decision" note, for why the corpus needed a
dedicated fixture here rather than continuing to rely on orthogonal
coverage through `agent/orchestrator.py` alone). Every other task in this
corpus can, in principle, be solved in one pass: the public tests are a
single, undifferentiated signal, and passing them plus the hidden overlay
is one act of synthesis. This task is built so that reading a static
problem statement and public test failures once is *not* enough — a
correct repair only follows from noticing that an already-green public
suite (produced by a first, incomplete fix) still hides a second, distinct
defect, and from separately confirming an unrelated, already-correct
function was left untouched. Acting on that feedback is the content this
task measures, not just its own arithmetic.

## Problem and oracle

`process_batch(b0, a1, a2, a3, a4, a5)` applies five signed adjustments, in
order, to an account balance bounded to `[0, 500]`. Each individual step
must:

1. Charge a flat handling fee of `3` when the adjustment is a **withdrawal**
   (`adjustment < 0`); a **deposit** (`adjustment >= 0`) is never charged a
   fee.
2. Fold the adjustment and its fee into the balance, **and only then**
   clamp the whole result back into `[0, 500]`.

The required per-step oracle is therefore:

```
fee(adjustment)        = 3 if adjustment < 0 else 0
apply_step(bal, adj)   = clamp(bal + adj - fee(adj), 0, 500)
process_batch(b0, a1..a5) = apply_step applied five times in sequence, starting from b0
```

The candidate module also carries a second, independent, already-correct
function left by a "prior session" and explicitly out of scope for this
repair:

```
tier_label(balance) = 0 if balance < 100, else 1 if balance < 300, else 2
```

## The iterative-repair narrative (read this before authoring the fix)

This section is itself part of the task statement every port's reference
implementation had to satisfy — not decoration. It reproduces, in redacted
form, the two-step debugging session that produced the correct oracle
above, exactly the shape of feedback a solver working this task for real
would receive across attempts.

**Attempt 0** (the naive first draft) charges the withdrawal fee
unconditionally, on every adjustment regardless of sign:

```
apply_step(bal, adj) = clamp(bal + adj - 3, 0, 500)   // wrong: fee on deposits too
```

Running the public suite against attempt 0 produces:

```
FAIL deposit_only_sequence_never_charges_a_fee
  process_batch(0, 50, 50, 50, 50, 50): expected 250, got 235
FAIL a_deposit_clamps_at_the_ceiling_with_no_fee_involved
  process_batch(480, 50, 0, 0, 0, 0): expected 500, got 488
ok  withdrawals_mid_range_never_approach_the_floor
```

That log is enough to name the first defect precisely: a deposit is being
charged a fee it should never see. **Attempt 1** fixes exactly that —
conditioning the fee on the adjustment's sign — and nothing else:

```
apply_step(bal, adj) = clamp(bal + adj, 0, 500) - fee(adj)   // fee now conditional, but subtracted after the clamp
```

Running the public suite again against attempt 1 produces:

```
ok  deposit_only_sequence_never_charges_a_fee
ok  withdrawals_mid_range_never_approach_the_floor
ok  a_deposit_clamps_at_the_ceiling_with_no_fee_involved
```

All green. A one-shot solver stops here. But this task's grader also runs a
hidden suite attempt 1 was never shown, and it still reports two failures,
redacted to their class rather than their exact vectors (the same
held-out discipline `bounded-counter-repair-v1/EQUIVALENCE.md` already
states for its own hidden vectors — see "Held-out discipline" below):

```
FAIL ledger_floor_invariant (2 case(s)): a returned balance violated the
     declared [0, 500] bound on a sequence ending in a withdrawal
ok   tier_classifier_preserved (3 case(s))
```

That redacted report is the second piece of feedback a solver must read
and act on to proceed: the public suite gives no signal about it at all
(every public vector deliberately avoids letting a withdrawal's fee
interact with the floor), so the only way to find the remaining defect is
to reason about *where* subtracting a fee after a clamp could still violate
the clamp's own invariant. The answer is the order of operations in
attempt 1's `apply_step`: clamping *before* subtracting the fee can return
a value below `0` whenever the pre-fee sum is already at or near the floor.
The fix is exactly the oracle stated above — fold the fee in *before*
clamping — which is what every reference port below implements.

The second redacted line (`tier_classifier_preserved`, all passing) is the
sibling-preservation control: it confirms that whatever produced the fix
above did not also touch `tier_label`, the prior session's unrelated,
already-correct function. A solver that "cleans up" the whole file while
fixing `apply_step` and inadvertently rewrites or deletes `tier_label` would
turn that second line into a failure too, even though its actual ledger fix
was correct — the discriminating case `stale-edit-preservation-v1` already
established for this corpus, reused here alongside the floor-invariant bug
rather than in isolation.

## Why this needs two defects, not one

A single masked defect (as in `bounded-counter-repair-v1` or
`concurrent-delta-merge-v1`) already proves a solver cannot succeed by
eyeballing the public suite alone. This task adds a second, independent
layer specifically to prove the property issue #211's "Agent workflow"
category asks for: a solver must act on *two rounds* of feedback framed
differently (an ordinary test failure, then a redacted grader report) and
must separately verify a preservation requirement neither round's raw
pass/fail count would surface on its own. Fixing defect 1 without reading
past the public suite's newly-green state, or fixing defect 2 while
ignoring the sibling function, both stop short of the fully corrected
behavior this task's hidden suite checks.

## Inputs and outputs

- **Inputs**: six signed 64-bit integers per `process_batch` vector (`b0`
  and five adjustments) and one signed 64-bit integer per `tier_label`
  vector, all literal fixtures baked into the source — no file, stdin,
  environment, or network input, the same discipline every task in this
  suite follows.
- **Outputs**: one signed 64-bit integer per call, exact equality.
- **Boundary of the measured region**: exactly `process_batch` (and the
  `clamp`/`fee`/`apply_step` helpers it calls) and `tier_label`, invoked
  in-process by each language's own official test runner. Process startup,
  compiler invocation, and test-harness overhead are outside the boundary
  and are never scored in this schema version (see `docs/METHODOLOGY.md`'s
  "No timing" section).
- **Allowed optimizations**: ordinary arithmetic, comparisons, and
  control flow only. No delegating `clamp`/saturating-arithmetic to a
  standard-library helper that would substitute for the algorithm (the same
  convention every task in this suite follows, enforced by author
  discipline and review — see `docs/METHODOLOGY.md`'s "What the harness
  cannot yet check").
- Overflow: every intermediate `balance + adjustment - fee` in every vector
  (public and hidden) stays well inside `i64`/`number` range before
  clamping, so no implementation needs an integer-overflow policy to pass.

## Public vectors (do not expose either masked defect)

| `b0` | adjustments | correct `process_batch` | why it stays silent about defect 2 |
| --- | --- | --- | --- |
| `0` | `50,50,50,50,50` | `250` | all deposits; fee never applies, so defect 2's fee-ordering bug cannot manifest — this vector's job is only to catch defect 1 |
| `400` | `50,0,0,0,0` | `450` | single deposit baseline |
| `200` | `-10,-10,-10,-10,-10` | `135` | all withdrawals, but the running balance never comes near `0` or `500`, so `clamp` is the identity at every step and the fee-ordering bug is invisible |
| `480` | `50,0,0,0,0` | `500` | hits the ceiling, but only via a deposit (fee `0` either way), so defect 2 (which only ever affects the floor — see below) cannot manifest |

`tier_label` is never called by the public suite at all, matching
`stale-edit-preservation-v1`'s convention for its own untouched sibling
function.

## Hidden vectors (only a fully correct implementation passes)

`hidden/<language>/` holds a second copy of the assertion entry point that
overlays (replaces, same relative path) the public one for scoring only; the
candidate module itself is untouched between phases.

- `process_batch(3, 0, 0, 0, 0, -3) == 0`: attempt 1's buggy order computes
  `clamp(3 + -3, 0, 500) - 3 = clamp(0, 0, 500) - 3 = 0 - 3 = -3`, one below
  the declared floor. The correct order computes
  `clamp(3 + -3 - 3, 0, 500) = clamp(-3, 0, 500) = 0`.
- `process_batch(53, 0, 0, 0, 0, -51) == 0`: attempt 1's buggy order computes
  `clamp(53 + -51, 0, 500) - 3 = clamp(2, 0, 500) - 3 = 2 - 3 = -1` — note
  the pre-fee sum (`2`) never even reaches the clamp's floor, so this case
  shows the bug is about fee-vs-clamp ordering, not merely about a sum that
  was already out of range. The correct order computes
  `clamp(53 + -51 - 3, 0, 500) = clamp(-1, 0, 500) = 0`.
- `tier_label(50) == 0`, `tier_label(150) == 1`, `tier_label(350) == 2`: the
  sibling-preservation control.

Both `process_batch` hidden vectors place the violating step **last** in
the sequence deliberately: an earlier violation is masked, because the next
step's own `clamp(balance + adjustment)` call floors a negative carried-over
balance back to `0` regardless of that step's own adjustment — so only a
violation in the final step survives to the value the harness actually
reads. All five were hand-verified against the exact attempt-1 formula
above before being committed (see "Split and negative control" below).

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports each keep `process_batch` and
`tier_label` in the same candidate module. Public and hidden phases share
that module unchanged; the hidden overlay replaces only the assertion
entry point (`main.rs` / `index.ts` / `src/app.spx`), the same overlay
style `concurrent-delta-merge-v1` and `stale-edit-preservation-v1` use.

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

## Split and negative control

This task is `split: "held_out"` in `tasks.json` and shares no fixture,
vector, or task family with any development or validation task in this
suite. Three independent wrong candidates were verified by hand against all
three real toolchains while authoring this task
(`tests/documentation/cross_language_benchmark_suite/iterative_repair_workflow.rs`
exercises them as regression controls):

1. **Attempt 0** (unconditional fee): fails multiple public vectors
   immediately, exactly as the narrative log above shows — included as a
   sanity control, not a hidden-test proof.
2. **Attempt 1** (fee conditional on sign, but subtracted after the clamp):
   passes every public vector and fails both `process_batch` hidden
   vectors with the exact divergent values named above (`-3` instead of
   `0`, then `-1` instead of `0`), while still passing the `tier_label`
   hidden checks.
3. **Sibling-broken candidate** (the floor-ordering bug fully fixed, but
   `tier_label`'s branch order swapped so it returns `2` for a low balance
   and `0` for a high one): passes every public vector and every
   `process_batch` hidden vector, and fails only the three `tier_label`
   hidden checks — proving the sibling-preservation control is
   independently discriminating, not redundant with the floor-invariant
   fix.

No trial, correctness score, or repair transcript for this task may be
exported for training or tuning reuse while it remains held out, for the
same reason `bounded-counter-repair-v1/EQUIVALENCE.md` states for its own
held-out declaration. It carries its own `task_family`
(`iterative-repair-workflow-v1`), distinct from every other task in this
corpus.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension, following
`concurrent-delta-merge-v1/EQUIVALENCE.md`'s established convention for
this corpus. C, Python, and Java keep the same candidate/entry split the
Rust and TypeScript ports use (`candidate.c`/`candidate.py`/
`Candidate.java` hold the unchanged `process_batch`/`tier_label`
functions; the hidden overlay replaces only `main.c`/`digest.py`/
`Main.java`, via `#include`, `import`, and javac's same-directory
auto-discovery respectively). Swift's fixed single-file `swiftc
main.swift` invocation admits no such split, so every function is repeated
verbatim in both the public and hidden `main.swift`.

Each port was authored independently against this file's clamp-after-fee
contract and the Rust/TypeScript references, not transliterated
line-by-line, and was independently compiled/run against the public and
hidden vectors above, then checked against attempt 1's negative control
(fee conditioned correctly but subtracted after the clamp, in place of
folding it in before clamping): the mutant passes every public vector and
fails both `process_batch` hidden vectors with the exact divergent values
this file names above (`-3`/`-1` instead of the correct `0`/`0`) in all
four languages, confirming the hidden vectors are non-vacuous.
