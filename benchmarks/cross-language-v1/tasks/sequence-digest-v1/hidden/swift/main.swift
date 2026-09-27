// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only, adding two hidden vectors a solver never
// sees. Implementation functions are unchanged from the public file.
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

// Hidden cases: never shipped in the public directory tree.
check(total(0, 0, 0, 0, 0), 0, "sum(0,0,0,0,0)")
check(countEven(0, 0, 0, 0, 0), 5, "countEven(0,0,0,0,0)")
check(countNegative(0, 0, 0, 0, 0), 0, "countNegative(0,0,0,0,0)")
check(maxOf(0, 0, 0, 0, 0), 0, "maxOf(0,0,0,0,0)")

check(total(-100, 7, 7, 7, 100), 21, "sum(-100,7,7,7,100)")
check(countEven(-100, 7, 7, 7, 100), 2, "countEven(-100,7,7,7,100)")
check(countNegative(-100, 7, 7, 7, 100), 1, "countNegative(-100,7,7,7,100)")
check(maxOf(-100, 7, 7, 7, 100), 100, "maxOf(-100,7,7,7,100)")

exit(failures == 0 ? 0 : 1)
