// Public implementation: a saturating counter (bounded to [0, 100]) that
// applies five signed integer deltas in sequence, clamping after every
// individual step rather than only once at the end. See
// ../../EQUIVALENCE.md for the exact input/output/boundary contract every
// language implementation of this task must meet.
//
// Compiled and run directly with `swiftc`, exactly as sequence-digest-v1's
// own Swift port already establishes for this suite's Swift lane.
import Foundation

func clamp(_ value: Int64) -> Int64 {
    if value > 100 {
        return 100
    } else if value < 0 {
        return 0
    } else {
        return value
    }
}

func step(_ counter: Int64, _ delta: Int64) -> Int64 {
    clamp(counter + delta)
}

func apply5(_ c0: Int64, _ d1: Int64, _ d2: Int64, _ d3: Int64, _ d4: Int64, _ d5: Int64) -> Int64 {
    step(step(step(step(step(c0, d1), d2), d3), d4), d5)
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(apply5(0, 10, 10, 10, 10, 10), 50, "apply5(0,10,10,10,10,10)")
check(apply5(50, 10, -5, 10, -5, 10), 70, "apply5(50,10,-5,10,-5,10)")
check(apply5(95, 10, 0, 0, 0, 0), 100, "apply5(95,10,0,0,0,0)")
check(apply5(5, -10, 0, 0, 0, 0), 0, "apply5(5,-10,0,0,0,0)")

exit(failures == 0 ? 0 : 1)
