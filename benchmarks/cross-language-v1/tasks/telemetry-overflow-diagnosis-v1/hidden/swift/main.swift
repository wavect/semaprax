// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `combineTelemetry` is unchanged from
// the public file. A delta that would carry the total past the 32-bit
// signed boundary must saturate instead of wrapping or trapping.
import Foundation

func combineTelemetry(_ deltaA: Int32, _ deltaB: Int32) -> Int32 {
    if deltaB > 0 && deltaA > Int32.max - deltaB {
        return Int32.max
    } else if deltaB < 0 && deltaA < Int32.min - deltaB {
        return Int32.min
    } else {
        return deltaA + deltaB
    }
}

var failures: Int32 = 0

func check(_ actual: Int32, _ expected: Int32, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(combineTelemetry(Int32.max, 1), Int32.max, "saturates at the positive boundary")
check(combineTelemetry(Int32.min, -1), Int32.min, "saturates at the negative boundary")

exit(failures == 0 ? 0 : 1)
