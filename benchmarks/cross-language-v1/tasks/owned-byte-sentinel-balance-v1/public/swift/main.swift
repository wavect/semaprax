// Public implementation and test entry: map sentinels in an owned byte
// buffer and compute the one-based positional checksum of the transformed
// bytes. See ../../EQUIVALENCE.md for the exact contract.
//
// `swiftc` is invoked with only this one file, so unlike the C/Python/Java
// ports this task's function cannot be split into a separate unchanged
// module; the hidden overlay below repeats it verbatim. `input` is copied
// into a private, owned `[UInt8]` before the sentinel mapping is applied in
// place, mirroring the Rust reference's `input.to_vec()`.
import Foundation

func sentinelChecksum(_ input: [UInt8]) -> Int64 {
    var transformed = input
    for index in 0..<transformed.count {
        let byte = transformed[index]
        if byte == 0xff {
            transformed[index] = 0x00
        } else if byte == 0x00 {
            transformed[index] = 0xff
        } else {
            transformed[index] = 0x01
        }
    }
    var checksum: Int64 = 0
    for index in 0..<transformed.count {
        checksum += Int64(index + 1) * Int64(transformed[index])
    }
    return checksum
}

var failures: Int32 = 0

func check(_ actual: Int64, _ expected: Int64, _ label: String) {
    if actual != expected {
        FileHandle.standardError.write("\(label): expected \(expected), got \(actual)\n".data(using: .utf8)!)
        failures += 1
    }
}

check(sentinelChecksum([0xff]), 0, "one ff")
check(sentinelChecksum([0xff, 0x07, 0xff]), 2, "two ff")
check(sentinelChecksum([0x01, 0x02, 0x7f]), 6, "other bytes")

exit(failures == 0 ? 0 : 1)
