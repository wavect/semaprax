// Public implementation and test entry: a five-step account-ledger repair
// with a withdrawal fee, plus a second, already-correct tierLabel
// classifier left by a prior session (out of scope for this repair). See
// ../../EQUIVALENCE.md for the full iterative-repair narrative this task's
// public/hidden split is built around.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's functions cannot be split into a separate unchanged
// module; the hidden overlay below repeats them verbatim.
import Foundation

func clampBalance(_ value: Int64) -> Int64 {
    if value < 0 {
        return 0
    } else if value > 500 {
        return 500
    } else {
        return value
    }
}

// A withdrawal (a negative adjustment) is charged a flat handling fee; a
// deposit (zero or positive) is not.
func fee(_ adjustment: Int64) -> Int64 {
    return adjustment < 0 ? 3 : 0
}

// One step: fold the adjustment and its fee into the balance, THEN clamp
// the whole result. Subtracting the fee after clamping is the second,
// masked defect this task exists to catch.
func applyStep(_ balance: Int64, _ adjustment: Int64) -> Int64 {
    return clampBalance(balance + adjustment - fee(adjustment))
}

func processBatch(_ b0: Int64, _ a1: Int64, _ a2: Int64, _ a3: Int64, _ a4: Int64, _ a5: Int64) -> Int64 {
    return applyStep(applyStep(applyStep(applyStep(applyStep(b0, a1), a2), a3), a4), a5)
}

// prior-session: account tier classifier, unrelated to the ledger repair
// above; keep unchanged.
func tierLabel(_ balance: Int64) -> Int64 {
    if balance < 100 {
        return 0
    } else if balance < 300 {
        return 1
    } else {
        return 2
    }
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

// Public tests: four vectors that never let a withdrawal's fee interact
// with the balance floor.
check(processBatch(0, 50, 50, 50, 50, 50), 250, "deposit-only sequence never charges a fee")
check(processBatch(400, 50, 0, 0, 0, 0), 450, "single deposit baseline")
check(processBatch(200, -10, -10, -10, -10, -10), 135, "withdrawals mid-range, never approach the floor")
check(processBatch(480, 50, 0, 0, 0, 0), 500, "a deposit clamps at the ceiling with no fee involved")

exit(failures == 0 ? 0 : 1)
