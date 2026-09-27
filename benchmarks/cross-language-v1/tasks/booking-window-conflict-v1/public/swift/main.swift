// Public implementation and test entry: two half-open booking windows
// conflict exactly when they share at least one instant. See
// ../../EQUIVALENCE.md for the exact contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's predicate cannot be split into a separate unchanged
// module; the hidden overlay below repeats it verbatim.
import Foundation

func conflicts(_ aStart: Int64, _ aEnd: Int64, _ bStart: Int64, _ bEnd: Int64) -> Int64 {
    (aStart < bEnd && bStart < aEnd) ? 1 : 0
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(conflicts(10, 20, 15, 25), 1, "proper overlap")
check(conflicts(10, 30, 12, 18), 1, "contained booking")
check(conflicts(10, 20, 25, 30), 0, "strictly separated bookings")

exit(failures == 0 ? 0 : 1)
