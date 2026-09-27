// The starter calculator module a fresh scaffold ships with one operation
// (`add`); this port's baseline mirrors that same minimal two-function
// calculator shape a fresh project would start from before any of its own
// logic exists. Public test entry: the starter operation and its new
// sibling compute correctly. See ../../EQUIVALENCE.md for the exact
// contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's functions cannot be split into a separate unchanged
// module; the hidden overlay below repeats them verbatim.
import Foundation

func add(_ left: Int64, _ right: Int64) -> Int64 {
    left + right
}

func subtract(_ left: Int64, _ right: Int64) -> Int64 {
    left - right
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(add(19, 23), 42, "the starter operation still works")
check(subtract(50, 8), 42, "the new sibling operation")

exit(failures == 0 ? 0 : 1)
