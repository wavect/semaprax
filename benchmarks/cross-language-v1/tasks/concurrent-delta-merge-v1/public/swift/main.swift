// Public implementation and test entry: merge two independently-arriving
// deltas against a shared [0, 1_000_000]-bounded counter from one common
// base value, treating both deltas as arriving concurrently. See
// ../../EQUIVALENCE.md for the exact contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's function cannot be split into a separate unchanged
// module; the hidden overlay below repeats it verbatim.
import Foundation

func mergeConcurrentDeltas(_ base: Int64, _ deltaA: Int64, _ deltaB: Int64) -> Int64 {
    let total = base + deltaA + deltaB
    if total < 0 {
        return 0
    } else if total > 1_000_000 {
        return 1_000_000
    } else {
        return total
    }
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(mergeConcurrentDeltas(100, 50, -30), 120, "well inside the bound")
check(mergeConcurrentDeltas(500_000, 100, 100), 500_200, "midrange sum")
check(mergeConcurrentDeltas(10, -5, -3), 2, "negative deltas away from the floor")
check(mergeConcurrentDeltas(0, 0, 0), 0, "identity")

exit(failures == 0 ? 0 : 1)
