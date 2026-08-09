import Foundation
import SemapraxPrivateSwift

struct ContractFailure: Error, CustomStringConvertible, Sendable {
    let description: String
}

@inline(__always)
func requireContract(_ condition: @autoclosure () -> Bool, _ message: String) throws {
    if !condition() { throw ContractFailure(description: message) }
}

struct StatusWord: Equatable, Sendable {
    static let adapterKAT: UInt64 = 0x0000002d00000001
    static let wrongThreadKAT: UInt64 = 0x0000002d00000002
    static let liveSessionsKAT: UInt64 = 0x0000002d00000005
    static let invalidHandleKAT: UInt64 = 0x0000002d00000007
    static let staleHandleKAT: UInt64 = 0x0000002d00000008
    static let crossRuntimeKAT: UInt64 = 0x0000002d00000009
    static let alreadyOpenKAT: UInt64 = 0x0000002d0000000a
    static let reentrantKAT: UInt64 = 0x0000002d0000000b
    static let uncertaintyKAT: UInt64 = 0x0000002d80000001
    static let panicKAT: UInt64 = 0x0000002d80000002
    static let unhealthyKAT: UInt64 = 0x0000002d80000003
    static let evidenceFailureKAT: UInt64 = 0x0000002d80000004

    let raw: UInt64
    var isSuccess: Bool { raw == 0 }
    var code: UInt32 { UInt32(truncatingIfNeeded: raw) }
    var statusClass: UInt64 { (raw >> 32) & 0x7 }
    var retryability: UInt64 { (raw >> 35) & 0x3 }
    var domainOrdinal: UInt64 { (raw >> 37) & 0xffff }
    var isPrecommitAppleRejection: Bool {
        !isSuccess && domainOrdinal == 1 && (code & 0x8000_0000) == 0
    }

    func requireWellFormed() throws {
        if isSuccess { return }
        try requireContract(raw >> 53 == 0, "status reserved bits are nonzero")
        try requireContract(code != 0, "nonzero status has a zero code")
        try requireContract((1...5).contains(statusClass), "status class is outside SPXAJS01")
        try requireContract((0...2).contains(retryability), "retryability is outside SPXAJS01")
        try requireContract(domainOrdinal != 0, "status domain is zero")
    }

    static func encode(code: UInt32, statusClass: UInt64, retryability: UInt64,
                       domainOrdinal: UInt64) -> UInt64 {
        UInt64(code) | (statusClass << 32) | (retryability << 35) | (domainOrdinal << 37)
    }

    static func requireKnownAnswers() throws {
        try requireContract(
            encode(code: 1, statusClass: 5, retryability: 1, domainOrdinal: 1) == adapterKAT,
            "SPXAJS01 encode KAT changed")
        let decoded = StatusWord(raw: adapterKAT)
        try decoded.requireWellFormed()
        try requireContract(decoded.code == 1 && decoded.statusClass == 5 &&
                            decoded.retryability == 1 && decoded.domainOrdinal == 1,
                            "SPXAJS01 decode KAT changed")
        for terminal in [uncertaintyKAT, panicKAT, unhealthyKAT, evidenceFailureKAT] {
            try requireContract(!StatusWord(raw: terminal).isPrecommitAppleRejection,
                                "terminal status became retryable")
        }
        let hostileZeroCode = StatusWord(raw: encode(
            code: 0, statusClass: 5, retryability: 1, domainOrdinal: 1))
        do {
            try hostileZeroCode.requireWellFormed()
            throw ContractFailure(description: "zero-code hostile status was accepted")
        } catch let failure as ContractFailure {
            try requireContract(failure.description == "nonzero status has a zero code",
                                "zero-code hostile status failed for the wrong reason")
        }
    }
}

struct OpaqueHandle: Equatable, Sendable {
    static let knownAnswer: UInt64 = 0x0001000001000001
    let raw: UInt64
    let runtimeTag: UInt64
    let generation: UInt64
    let slot: UInt64

    init(_ raw: UInt64) throws {
        let tag = (raw >> 48) & 0x7fff
        let generation = (raw >> 24) & 0x00ff_ffff
        let slot = raw & 0x00ff_ffff
        try requireContract(raw != 0 && raw >> 63 == 0 && tag != 0 && generation != 0 && slot != 0,
                            "invalid SPXAJH01 handle")
        self.raw = raw
        self.runtimeTag = tag
        self.generation = generation
        self.slot = slot
    }

    static func encode(runtimeTag: UInt64, generation: UInt64, slot: UInt64) -> UInt64 {
        (runtimeTag << 48) | (generation << 24) | slot
    }

    static func requireKnownAnswer() throws {
        try requireContract(encode(runtimeTag: 1, generation: 1, slot: 1) == knownAnswer,
                            "SPXAJH01 encode KAT changed")
        let decoded = try OpaqueHandle(knownAnswer)
        try requireContract(decoded.runtimeTag == 1 && decoded.generation == 1 && decoded.slot == 1,
                            "SPXAJH01 decode KAT changed")
    }
}

struct ConsumeEvidence: Equatable, Sendable {
    static let poison: UInt64 = 0xa5a5_a5a5_a5a5_a5a5
    static let byteCount: UInt32 = 64
    let words: [UInt64]

    func requireExact() throws {
        try requireContract(words.count == 8, "evidence width changed")
        try requireContract(words[0] == 1, "evidence version changed")
        try requireContract(words[1] != 0, "module instance identity is zero")
        try requireContract(words[2] == 0x0f, "proof flags changed")
        try requireContract(words[3] == 0, "postcommit allocation count is nonzero")
        try requireContract(words[4] == 2, "finalizer count changed")
        try requireContract(words[5] == 0x000000010000000d, "first finalizer changed")
        try requireContract(words[6] == 0x000000000000000b, "second finalizer changed")
        try requireContract(words[7] == 0, "host flags changed")
    }

    func requirePoisoned() throws {
        try requireContract(words == Array(repeating: Self.poison, count: 8),
                            "native rejection modified output evidence")
    }
}

func poisonedEvidence() -> spx_private_apple_swift_evidence_v1 {
    var value = spx_private_apple_swift_evidence_v1()
    withUnsafeMutableBytes(of: &value) { bytes in
        for index in 0..<8 { bytes.storeBytes(of: ConsumeEvidence.poison, toByteOffset: index * 8, as: UInt64.self) }
    }
    return value
}

func evidenceWords(_ value: inout spx_private_apple_swift_evidence_v1) -> [UInt64] {
    withUnsafeBytes(of: &value) { Array($0.bindMemory(to: UInt64.self)) }
}
