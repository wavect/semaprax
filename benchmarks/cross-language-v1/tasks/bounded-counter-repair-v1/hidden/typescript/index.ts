// Hidden overlay: replaces the public `index.ts` verbatim (same relative
// path) for the scoring phase only, adding hidden assertions that exercise
// the classic off-by-one repair bug this task is about: clamping only the
// final summed delta instead of clamping the running counter after every
// individual step. Implementation functions are unchanged from the public
// file.

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

// Hidden cases: never shipped in the public directory tree. A single
// end-of-sequence clamp instead of a per-step clamp gets both of these
// wrong (100 and 35, respectively); the correct stepwise counter clamps
// after every delta and gets 70 and 50.
assertEqual(
  apply5(90, 50, -30, 0, 0, 0),
  70,
  "apply5(90,50,-30,0,0,0) saturates upward then recovers downward"
);
assertEqual(
  apply5(5, -20, 50, 0, 0, 0),
  50,
  "apply5(5,-20,50,0,0,0) floors downward then recovers upward"
);

console.log("ok");
