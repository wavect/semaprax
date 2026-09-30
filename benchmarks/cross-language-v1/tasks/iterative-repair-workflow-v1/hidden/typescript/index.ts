import { processBatch, tierLabel } from "./candidate";

function assertEqual(actual: number, expected: number, label: string): void {
  if (actual !== expected) throw new Error(label + ": expected " + expected + ", got " + actual);
}

// Hidden overlay: replaces the public `index.ts` verbatim (same relative
// path) for the scoring phase only, importing the unchanged public
// `candidate.ts`. Adds the two floor-interaction vectors that only a
// correctly-ordered ("clamp last") repair passes, plus three checks on the
// unrelated `tierLabel` classifier the public suite never calls at all.
assertEqual(processBatch(0, 50, 50, 50, 50, 50), 250, "deposit-only sequence never charges a fee");
assertEqual(processBatch(400, 50, 0, 0, 0, 0), 450, "single deposit baseline");
assertEqual(processBatch(200, -10, -10, -10, -10, -10), 135, "withdrawals mid-range, never approach the floor");
assertEqual(processBatch(480, 50, 0, 0, 0, 0), 500, "a deposit clamps at the ceiling with no fee involved");

// A withdrawal fee subtracted after the balance is clamped, instead of
// before, can return a balance below the declared floor whenever the
// violating step is the sequence's last one. `attempt_1` in
// EQUIVALENCE.md's narrative fixes the deposit-fee defect but keeps this
// one, so it passes every public vector above and fails both of these.
assertEqual(
  processBatch(3, 0, 0, 0, 0, -3),
  0,
  "a final withdrawal that would cross the floor must fold its fee before clamping",
);
assertEqual(
  processBatch(53, 0, 0, 0, 0, -51),
  0,
  "a final withdrawal whose pre-fee sum is still in range must still fold its fee before clamping",
);

assertEqual(tierLabel(50), 0, "tier classifier: low balance");
assertEqual(tierLabel(150), 1, "tier classifier: mid balance");
assertEqual(tierLabel(350), 2, "tier classifier: high balance");
