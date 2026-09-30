// Public implementation and test entry: a telemetry combiner register must
// saturate at the 32-bit signed boundary rather than wrap or trap. See
// ../../EQUIVALENCE.md for the exact contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's function cannot be split into a separate unchanged
// module; the hidden overlay below repeats it verbatim. `Int32` gives the
// genuine 32-bit width `Int32.max`/`Int32.min` need; the guard below is
// evaluated before any addition that could overflow, so this never
// triggers Swift's trapping integer overflow.
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

check(combineTelemetry(10, 20), 30, "ordinary positive deltas")
check(combineTelemetry(-5, 5), 0, "ordinary mixed deltas")

exit(failures == 0 ? 0 : 1)
