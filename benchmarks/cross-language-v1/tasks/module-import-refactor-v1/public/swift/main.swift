// Public implementation and test entry: a bounded invoice total computed
// through a whole-subtotal tax helper. See ../../EQUIVALENCE.md for the
// exact contract.
//
// `swiftc` is invoked with only this one file (this suite's fixed adapter
// command), so unlike the C/Python/Java ports this task's helper cannot be
// split into a separate file; `Helper` and `Candidate` below are kept as
// distinct namespaces within this one file to preserve the same
// caller/imported-helper shape the task measures, without literal
// cross-file compilation.
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

check(Candidate.invoiceTotal(10, 2, 10, 5), 27, "exact tax with shipping")
check(Candidate.invoiceTotal(40, 2, 25, 0), 100, "exact tax without shipping")
check(Candidate.invoiceTotal(25, 4, 20, 10), 130, "larger exact subtotal")

exit(failures == 0 ? 0 : 1)
