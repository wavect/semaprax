// Public implementation and test entry: classify a bounded versioned record
// envelope, first-error precedence. See ../../EQUIVALENCE.md for the exact
// contract.
//
// `swiftc` is invoked with only this one file (this suite's fixed adapter
// command), so unlike the C/Python/Java ports this task's `validate` cannot
// be split into a separate unchanged-across-phases module; the hidden
// overlay below repeats it verbatim, exactly as `sequence-digest-v1`'s own
// Swift port already does for its own functions.
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

check(validate(7, 1, 1), 0, "minimum valid envelope")
check(validate(7, 1, 64), 0, "maximum valid envelope")
check(validate(6, 1, 10), 1, "unknown kind")
check(validate(7, 2, 10), 2, "unsupported version")
check(validate(7, 1, 0), 3, "short payload")
check(validate(7, 1, 65), 3, "long payload")

exit(failures == 0 ? 0 : 1)
