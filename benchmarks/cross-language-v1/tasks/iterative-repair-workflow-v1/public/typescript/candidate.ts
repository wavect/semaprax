// Candidate: a five-step account-ledger repair with a withdrawal fee, plus
// a second, already-correct `tierLabel` classifier left by a prior session
// (out of scope for this repair). See ../../EQUIVALENCE.md for the full
// iterative-repair narrative this task's public/hidden split is built
// around. Unchanged between the public and hidden phases.

function clamp(value: number): number {
  if (value < 0) return 0;
  if (value > 500) return 500;
  return value;
}

// A withdrawal (a negative adjustment) is charged a flat handling fee; a
// deposit (zero or positive) is not.
function fee(adjustment: number): number {
  return adjustment < 0 ? 3 : 0;
}

// One step: fold the adjustment and its fee into the balance, THEN clamp
// the whole result. Subtracting the fee after clamping is the second,
// masked defect this task exists to catch.
function applyStep(balance: number, adjustment: number): number {
  return clamp(balance + adjustment - fee(adjustment));
}

export function processBatch(
  b0: number,
  a1: number,
  a2: number,
  a3: number,
  a4: number,
  a5: number,
): number {
  return applyStep(
    applyStep(applyStep(applyStep(applyStep(b0, a1), a2), a3), a4),
    a5,
  );
}

// prior-session: account tier classifier, unrelated to the ledger repair
// above; keep unchanged.
export function tierLabel(balance: number): number {
  if (balance < 100) return 0;
  if (balance < 300) return 1;
  return 2;
}
