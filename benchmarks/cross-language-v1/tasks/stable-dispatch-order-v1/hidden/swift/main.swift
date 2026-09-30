// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `dispatchOrder` is unchanged from the
// public file. Ties must preserve arrival order, including a three-way
// tie.
import Foundation

func dispatchOrder(_ aPriority: Int64, _ bPriority: Int64, _ cPriority: Int64) -> Int64 {
    if aPriority <= bPriority && aPriority <= cPriority {
        return bPriority <= cPriority ? 123 : 132
    }
    if bPriority <= aPriority && bPriority <= cPriority {
        return aPriority <= cPriority ? 213 : 231
    }
    return aPriority <= bPriority ? 312 : 321
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(dispatchOrder(1, 1, 2), 123, "first two arrivals tie")
check(dispatchOrder(1, 2, 1), 132, "first and last arrivals tie")
check(dispatchOrder(2, 1, 1), 231, "last two arrivals tie")
check(dispatchOrder(7, 7, 7), 123, "all arrivals tie")

exit(failures == 0 ? 0 : 1)
