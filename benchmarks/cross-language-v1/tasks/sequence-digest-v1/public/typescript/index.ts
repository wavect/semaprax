// Public implementation: four independent scalar digests over five signed
// integer inputs. See ../../EQUIVALENCE.md for the exact input/output/
// boundary contract every language implementation of this task must meet.
//
// Compiled with `tsc --strict` and executed directly with `node`. Assertions
// are a five-line local helper rather than `node:assert`, because typing
// `node:assert`'s import requires the `@types/node` package and this
// snapshot installs no packages (no network access at build time); a failed
// assertion throws, and an uncaught throw is Node's own nonzero-exit signal.

function assertEqual(actual: number, expected: number, label: string): void {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${expected}, got ${actual}`);
  }
}

function isEven(value: number): number {
  return value % 2 === 0 ? 1 : 0;
}

function isNegative(value: number): number {
  return value < 0 ? 1 : 0;
}

function max2(left: number, right: number): number {
  return left > right ? left : right;
}

function sum(a: number, b: number, c: number, d: number, e: number): number {
  return a + b + c + d + e;
}

function countEven(a: number, b: number, c: number, d: number, e: number): number {
  return isEven(a) + isEven(b) + isEven(c) + isEven(d) + isEven(e);
}

function countNegative(a: number, b: number, c: number, d: number, e: number): number {
  return isNegative(a) + isNegative(b) + isNegative(c) + isNegative(d) + isNegative(e);
}

function maxOf(a: number, b: number, c: number, d: number, e: number): number {
  return max2(max2(max2(a, b), max2(c, d)), e);
}

assertEqual(sum(1, 2, 3, 4, 5), 15, "sum(1,2,3,4,5)");
assertEqual(countEven(1, 2, 3, 4, 5), 2, "countEven(1,2,3,4,5)");
assertEqual(countNegative(1, 2, 3, 4, 5), 0, "countNegative(1,2,3,4,5)");
assertEqual(maxOf(1, 2, 3, 4, 5), 5, "maxOf(1,2,3,4,5)");

assertEqual(sum(-1, -2, -3, -4, -5), -15, "sum(-1,-2,-3,-4,-5)");
assertEqual(countEven(-1, -2, -3, -4, -5), 2, "countEven(-1,-2,-3,-4,-5)");
assertEqual(countNegative(-1, -2, -3, -4, -5), 5, "countNegative(-1,-2,-3,-4,-5)");
assertEqual(maxOf(-1, -2, -3, -4, -5), -1, "maxOf(-1,-2,-3,-4,-5)");

console.log("ok");
