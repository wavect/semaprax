// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `conflicts` is unchanged from the
// public file. Forward and reverse adjacency (a shared boundary instant,
// not a shared interior instant) must not conflict.
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

check(conflicts(10, 20, 20, 25), 0, "forward adjacency")
check(conflicts(20, 25, 10, 20), 0, "reverse adjacency")

exit(failures == 0 ? 0 : 1)
