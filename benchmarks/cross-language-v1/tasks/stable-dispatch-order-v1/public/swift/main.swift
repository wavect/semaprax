// Public implementation and test entry: order three jobs by increasing
// priority while preserving arrival order for ties. See
// ../../EQUIVALENCE.md for the exact contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's ordering function cannot be split into a separate
// unchanged module; the hidden overlay below repeats it verbatim.
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

check(dispatchOrder(1, 2, 3), 123, "already ordered distinct priorities")
check(dispatchOrder(3, 1, 2), 231, "middle arrival has smallest priority")
check(dispatchOrder(2, 3, 1), 312, "last arrival has smallest priority")

exit(failures == 0 ? 0 : 1)
