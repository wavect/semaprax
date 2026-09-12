// Hidden overlay: replaces the public `index.ts` verbatim (same relative
// path) for the scoring phase only, adding hidden assertions a solver never
// sees. Implementation functions are unchanged from the public file.

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

// Hidden cases: never shipped in the public directory tree.
assertEqual(sum(0, 0, 0, 0, 0), 0, "sum(0,0,0,0,0)");
assertEqual(countEven(0, 0, 0, 0, 0), 5, "countEven(0,0,0,0,0)");
assertEqual(countNegative(0, 0, 0, 0, 0), 0, "countNegative(0,0,0,0,0)");
assertEqual(maxOf(0, 0, 0, 0, 0), 0, "maxOf(0,0,0,0,0)");

assertEqual(sum(-100, 7, 7, 7, 100), 21, "sum(-100,7,7,7,100)");
assertEqual(countEven(-100, 7, 7, 7, 100), 2, "countEven(-100,7,7,7,100)");
assertEqual(countNegative(-100, 7, 7, 7, 100), 1, "countNegative(-100,7,7,7,100)");
assertEqual(maxOf(-100, 7, 7, 7, 100), 100, "maxOf(-100,7,7,7,100)");

console.log("ok");
