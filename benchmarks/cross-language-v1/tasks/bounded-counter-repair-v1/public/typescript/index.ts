// Public implementation: a saturating counter (bounded to [0, 100]) that
// applies five signed integer deltas in sequence, clamping after every
// individual step rather than only once at the end. See
// ../../EQUIVALENCE.md for the exact input/output/boundary contract every
// language implementation of this task must meet.
//
// Compiled with `tsc --strict` and executed directly with `node`. Assertions
// are a five-line local helper rather than `node:assert`, matching
// sequence-digest-v1's own fixture: typing `node:assert`'s import needs the
// `@types/node` package and this snapshot installs no packages (no network
// access at build time).

function assertEqual(actual: number, expected: number, label: string): void {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${expected}, got ${actual}`);
  }
}

function clamp(value: number): number {
  if (value > 100) {
    return 100;
  } else if (value < 0) {
    return 0;
  } else {
    return value;
  }
}

function step(counter: number, delta: number): number {
  return clamp(counter + delta);
}

function apply5(
  c0: number,
  d1: number,
  d2: number,
  d3: number,
  d4: number,
  d5: number
): number {
  return step(step(step(step(step(c0, d1), d2), d3), d4), d5);
}

assertEqual(apply5(0, 10, 10, 10, 10, 10), 50, "apply5(0,10,10,10,10,10)");
assertEqual(apply5(50, 10, -5, 10, -5, 10), 70, "apply5(50,10,-5,10,-5,10)");
assertEqual(apply5(95, 10, 0, 0, 0, 0), 100, "apply5(95,10,0,0,0,0)");
assertEqual(apply5(5, -10, 0, 0, 0, 0), 0, "apply5(5,-10,0,0,0,0)");

console.log("ok");
