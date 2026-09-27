// Public implementation and test entry: release a shipment only when both
// independent sensor readings lie within their inclusive accepted bands.
// See ../../EQUIVALENCE.md for the exact contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's predicate cannot be split into a separate unchanged
// module; the hidden overlay below repeats it verbatim.
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

check(releaseAllowed(5, 100), 1, "both readings inside operating bands")
check(releaseAllowed(1, 94), 0, "both readings below operating bands")
check(releaseAllowed(9, 106), 0, "both readings above operating bands")

exit(failures == 0 ? 0 : 1)
