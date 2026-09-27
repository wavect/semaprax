import { processBatch } from "./candidate";

function assertEqual(actual: number, expected: number, label: string): void {
  if (actual !== expected) throw new Error(label + ": expected " + expected + ", got " + actual);
}

// Public tests: four vectors that never let a withdrawal's fee interact
// with the balance floor. See ../../EQUIVALENCE.md's iterative-repair
// narrative for why a candidate that passes every one of these is not yet
// done.
assertEqual(processBatch(0, 50, 50, 50, 50, 50), 250, "deposit-only sequence never charges a fee");
assertEqual(processBatch(400, 50, 0, 0, 0, 0), 450, "single deposit baseline");
assertEqual(processBatch(200, -10, -10, -10, -10, -10), 135, "withdrawals mid-range, never approach the floor");
assertEqual(processBatch(480, 50, 0, 0, 0, 0), 500, "a deposit clamps at the ceiling with no fee involved");
