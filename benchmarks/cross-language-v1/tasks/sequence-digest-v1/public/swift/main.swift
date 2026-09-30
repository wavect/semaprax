// Public implementation: four independent scalar digests over five signed
// 64-bit inputs. See ../../EQUIVALENCE.md for the exact input/output/boundary
// contract every language implementation of this task must meet.
//
// Compiled and run directly with `swiftc`, Swift's own official single-file
// workflow; there is no external test framework dependency to install (this
// sandbox performs no package installs). A failed check reports through the
// process's own exit code, exactly as the other ports do.
import Foundation

func isEven(_ value: Int64) -> Int64 { value % 2 == 0 ? 1 : 0 }
func isNegative(_ value: Int64) -> Int64 { value < 0 ? 1 : 0 }
func max2(_ left: Int64, _ right: Int64) -> Int64 { left > right ? left : right }

func total(_ a: Int64, _ b: Int64, _ c: Int64, _ d: Int64, _ e: Int64) -> Int64 { a + b + c + d + e }

func countEven(_ a: Int64, _ b: Int64, _ c: Int64, _ d: Int64, _ e: Int64) -> Int64 {
    isEven(a) + isEven(b) + isEven(c) + isEven(d) + isEven(e)
}

func countNegative(_ a: Int64, _ b: Int64, _ c: Int64, _ d: Int64, _ e: Int64) -> Int64 {
    isNegative(a) + isNegative(b) + isNegative(c) + isNegative(d) + isNegative(e)
}

func maxOf(_ a: Int64, _ b: Int64, _ c: Int64, _ d: Int64, _ e: Int64) -> Int64 {
    max2(max2(max2(a, b), max2(c, d)), e)
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(total(1, 2, 3, 4, 5), 15, "sum(1,2,3,4,5)")
check(countEven(1, 2, 3, 4, 5), 2, "countEven(1,2,3,4,5)")
check(countNegative(1, 2, 3, 4, 5), 0, "countNegative(1,2,3,4,5)")
check(maxOf(1, 2, 3, 4, 5), 5, "maxOf(1,2,3,4,5)")

check(total(-1, -2, -3, -4, -5), -15, "sum(-1,-2,-3,-4,-5)")
check(countEven(-1, -2, -3, -4, -5), 2, "countEven(-1,-2,-3,-4,-5)")
check(countNegative(-1, -2, -3, -4, -5), 5, "countNegative(-1,-2,-3,-4,-5)")
check(maxOf(-1, -2, -3, -4, -5), -1, "maxOf(-1,-2,-3,-4,-5)")

exit(failures == 0 ? 0 : 1)
