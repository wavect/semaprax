// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `releaseAllowed` is unchanged from the
// public file. A good reading in one sensor must never override a bad
// reading in the other, and the band edges are inclusive.
import Foundation

func releaseAllowed(_ coreTemperature: Int64, _ sealPressure: Int64) -> Int64 {
    (coreTemperature >= 2 && coreTemperature <= 8 && sealPressure >= 95 && sealPressure <= 105) ? 1 : 0
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(releaseAllowed(5, 106), 0, "good temperature cannot override bad pressure")
check(releaseAllowed(1, 100), 0, "good pressure cannot override bad temperature")
check(releaseAllowed(2, 95), 1, "lower inclusive edges")
check(releaseAllowed(8, 105), 1, "upper inclusive edges")

exit(failures == 0 ? 0 : 1)
