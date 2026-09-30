// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `add`/`subtract` are unchanged from the
// public file. Subtraction may produce a negative result, unlike a bounded
// counter elsewhere in this corpus.
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

check(subtract(8, 50), -42, "subtraction may go negative, unlike a bounded counter")
check(subtract(-5, -5), 0, "subtracting equal negatives")
check(add(19, 23), 42, "the starter operation is unchanged")

exit(failures == 0 ? 0 : 1)
