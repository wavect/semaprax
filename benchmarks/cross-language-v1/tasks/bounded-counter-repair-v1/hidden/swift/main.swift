// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only, adding hidden vectors that exercise the
// classic off-by-one repair bug this task is about: clamping only the final
// summed delta instead of clamping the running counter after every
// individual step. Implementation functions are unchanged from the public
// file.
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

// Hidden cases: never shipped in the public directory tree. A single
// end-of-sequence clamp instead of a per-step clamp gets both of these wrong
// (100 and 35, respectively); the correct stepwise counter clamps after
// every delta and gets 70 and 50.
check(apply5(90, 50, -30, 0, 0, 0), 70, "apply5(90,50,-30,0,0,0) saturates upward then recovers downward")
check(apply5(5, -20, 50, 0, 0, 0), 50, "apply5(5,-20,50,0,0,0) floors downward then recovers upward")

exit(failures == 0 ? 0 : 1)
