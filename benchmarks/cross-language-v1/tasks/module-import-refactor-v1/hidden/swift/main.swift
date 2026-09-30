// Hidden overlay: replaces the public `main.swift` verbatim (same relative
// path) for the scoring phase only. `Helper`/`Candidate` are unchanged from
// the public file. Vectors where the whole-subtotal tax must precede
// shipping and round (truncate) exactly once.
import Foundation

enum Helper {
    static func taxForSubtotal(_ subtotal: Int64, _ taxRate: Int64) -> Int64 {
        (subtotal * taxRate) / 100
    }
}

enum Candidate {
    static func invoiceTotal(_ price: Int64, _ quantity: Int64, _ taxRate: Int64, _ shipping: Int64) -> Int64 {
        let subtotal = price * quantity
        return subtotal + Helper.taxForSubtotal(subtotal, taxRate) + shipping
    }
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(Candidate.invoiceTotal(19, 3, 8, 10), 71, "whole subtotal tax before shipping")
check(Candidate.invoiceTotal(7, 5, 13, 9), 48, "tax rounds once")
check(Candidate.invoiceTotal(1, 64, 17, 3), 77, "bounded quantity")

exit(failures == 0 ? 0 : 1)
