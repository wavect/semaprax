// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `applyDiscount`/`staleNote` are
// unchanged from the public file. A corrupted percentage above full price
// must floor at zero, and the preexisting `staleNote` helper must be
// unchanged.
import Foundation

func staleNote(_ tag: Int64) -> Int64 {
    tag * 2 + 7
}

func applyDiscount(_ price: Int64, _ pct: Int64) -> Int64 {
    let raw = price - (price * pct) / 100
    return raw < 0 ? 0 : raw
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(applyDiscount(100, 150), 0, "corrupted percentage floors at zero")
check(applyDiscount(40, 130), 0, "corrupted percentage floors at zero (2)")
check(staleNote(5), 17, "preexisting stale helper unchanged")
check(staleNote(0), 7, "preexisting stale helper unchanged at zero")

exit(failures == 0 ? 0 : 1)
