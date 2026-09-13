import { validate } from "./validate";
function assertEqual(actual: number, expected: number, label: string): void {
  if (actual !== expected) throw new Error(label + ": expected " + expected + ", got " + actual);
}
assertEqual(validate(6, 2, 0), 1, "kind precedes version and length");
assertEqual(validate(7, 2, 0), 2, "version precedes length");
assertEqual(validate(7, 1, -1), 3, "negative payload");
assertEqual(validate(7, 1, 65), 3, "large payload");
assertEqual(validate(7, 1, 64), 0, "maximum valid payload");
assertEqual(validate(7, 1, 1), 0, "minimum valid payload");
