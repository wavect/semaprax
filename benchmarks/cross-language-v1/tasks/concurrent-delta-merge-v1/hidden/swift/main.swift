// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `mergeConcurrentDeltas` is unchanged
// from the public file. A ceiling- or floor-side delta must not clamp
// before the other concurrent delta lands.
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

// Hidden boundary vectors: never shipped in the public directory tree. A
// candidate that clamps deltaA against base before adding deltaB --
// treating the two concurrent deltas as a sequential edit -- diverges from
// the correct concurrent merge exactly here.
check(
    mergeConcurrentDeltas(999_990, 20, -50),
    999_960,
    "a ceiling-side delta must not clamp before the other concurrent delta lands"
)
check(
    mergeConcurrentDeltas(10, -20, 15),
    5,
    "a floor-side delta must not clamp before the other concurrent delta lands"
)

exit(failures == 0 ? 0 : 1)
