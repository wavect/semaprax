// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `validate` is unchanged from the public
// file; compound-invalid envelopes make the first-error precedence
// independently observable: a candidate that checks version before kind, or
// accepts an out-of-range length, fails here.
import Foundation

func validate(_ kind: Int64, _ version: Int64, _ payloadLen: Int64) -> Int64 {
    if kind != 7 {
        return 1
    }
    if version != 1 {
        return 2
    }
    if payloadLen < 1 || payloadLen > 64 {
        return 3
    }
    return 0
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(validate(6, 2, 0), 1, "kind precedes version and length")
check(validate(7, 2, 0), 2, "version precedes length")
check(validate(7, 1, -1), 3, "negative payload")
check(validate(7, 1, 65), 3, "large payload")
check(validate(7, 1, 64), 0, "maximum valid payload")
check(validate(7, 1, 1), 0, "minimum valid payload")

exit(failures == 0 ? 0 : 1)
