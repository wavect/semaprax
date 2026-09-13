import { validate } from "./validate";
function assertEqual(actual: number, expected: number, label: string): void {
  if (actual !== expected) throw new Error(label + ": expected " + expected + ", got " + actual);
}
assertEqual(validate(7, 1, 1), 0, "minimum valid envelope");
assertEqual(validate(7, 1, 64), 0, "maximum valid envelope");
assertEqual(validate(6, 1, 10), 1, "unknown kind");
assertEqual(validate(7, 2, 10), 2, "unsupported version");
assertEqual(validate(7, 1, 0), 3, "short payload");
assertEqual(validate(7, 1, 65), 3, "long payload");
