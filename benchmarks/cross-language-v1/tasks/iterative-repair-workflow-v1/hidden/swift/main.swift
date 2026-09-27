// Hidden overlay: replaces the public main.swift verbatim (same relative
// path) for the scoring phase only. clampBalance/fee/applyStep/
// processBatch/tierLabel are unchanged from the public file. Adds the two
// floor-interaction vectors that only a correctly-ordered ("clamp last")
// repair passes, plus three checks on the unrelated tierLabel classifier
// the public suite never calls at all.
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

func fee(_ adjustment: Int64) -> Int64 {
    return adjustment < 0 ? 3 : 0
}

func applyStep(_ balance: Int64, _ adjustment: Int64) -> Int64 {
    return clampBalance(balance + adjustment - fee(adjustment))
}

func processBatch(_ b0: Int64, _ a1: Int64, _ a2: Int64, _ a3: Int64, _ a4: Int64, _ a5: Int64) -> Int64 {
    return applyStep(applyStep(applyStep(applyStep(applyStep(b0, a1), a2), a3), a4), a5)
}

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

check(processBatch(0, 50, 50, 50, 50, 50), 250, "deposit-only sequence never charges a fee")
check(processBatch(400, 50, 0, 0, 0, 0), 450, "single deposit baseline")
check(processBatch(200, -10, -10, -10, -10, -10), 135, "withdrawals mid-range, never approach the floor")
check(processBatch(480, 50, 0, 0, 0, 0), 500, "a deposit clamps at the ceiling with no fee involved")

// A withdrawal fee subtracted after the balance is clamped, instead of
// before, can return a balance below the declared floor whenever the
// violating step is the sequence's last one. attempt_1 in
// EQUIVALENCE.md's narrative fixes the deposit-fee defect but keeps this
// one, so it passes every public vector above and fails both of these.
check(
    processBatch(3, 0, 0, 0, 0, -3),
    0,
    "a final withdrawal that would cross the floor must fold its fee before clamping"
)
check(
    processBatch(53, 0, 0, 0, 0, -51),
    0,
    "a final withdrawal whose pre-fee sum is still in range must still fold its fee before clamping"
)

check(tierLabel(50), 0, "tier classifier: low balance")
check(tierLabel(150), 1, "tier classifier: mid balance")
check(tierLabel(350), 2, "tier classifier: high balance")

exit(failures == 0 ? 0 : 1)
