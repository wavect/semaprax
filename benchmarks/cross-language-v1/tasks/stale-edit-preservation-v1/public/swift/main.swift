// Public implementation and test entry: floor a corrupted-percentage
// discount at zero without disturbing an unrelated, already-completed
// helper function. See ../../EQUIVALENCE.md for the exact contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's functions cannot be split into a separate unchanged
// module; the hidden overlay below repeats them verbatim.
import Foundation

// prior-session: shipment tag helper; unrelated to this task's repair, keep
// unchanged. A candidate that clobbers or deletes this while repairing
// `applyDiscount` below has not preserved a stale, already-completed edit.
func staleNote(_ tag: Int64) -> Int64 {
    tag * 2 + 7
}

func applyDiscount(_ price: Int64, _ pct: Int64) -> Int64 {
    // A corrupted upstream feed can send `pct` above 100; the result must
    // floor at zero rather than go negative.
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

check(applyDiscount(100, 10), 90, "ten percent off")
check(applyDiscount(200, 25), 150, "twenty five percent off")

exit(failures == 0 ? 0 : 1)
