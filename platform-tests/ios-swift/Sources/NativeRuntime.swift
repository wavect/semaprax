import Foundation
import SemapraxPrivateSwift

struct NativeCallFailure: Error, Sendable {
    let status: StatusWord
    let evidence: ConsumeEvidence
}

final class NativeRuntime: @unchecked Sendable {
    private let fifo = StableFifoThread(name: "semaprax-private-swift-owner")
    private let cleanupLock = NSLock()
    private var cleanupResults: [Result<ConsumeEvidence, Error>] = []

    func open() throws {
        let status = StatusWord(raw: try fifo.call { spx_private_apple_swift_fixture_v1_open() })
        try status.requireWellFormed()
        try requireContract(status.isSuccess, "private Swift fixture open failed")
    }

    func openAgainRaw() throws -> StatusWord {
        StatusWord(raw: try fifo.call { spx_private_apple_swift_fixture_v1_open() })
    }

    func openRequiresFalse() throws {
        let status = StatusWord(raw: try fifo.call {
            spx_private_apple_swift_fixture_rf_v1_open()
        })
        try status.requireWellFormed()
        try requireContract(status.isSuccess, "private Swift requires-false fixture open failed")
    }

    func openRequiresFalseAgainRaw() throws -> StatusWord {
        StatusWord(raw: try fifo.call { spx_private_apple_swift_fixture_rf_v1_open() })
    }

    func adopt() throws -> OwnedSession {
        let call: (StatusWord, UInt64) = try fifo.call {
            var output = UInt64.max
            let status = StatusWord(raw: spx_private_apple_swift_v1_adopt_pair(11, 13, &output))
            return (status, output)
        }
        let status = call.0
        let output = call.1
        try status.requireWellFormed()
        if !status.isSuccess {
            try requireContract(output == UInt64.max, "adopt rejection modified output handle")
            throw ContractFailure(description: "private Swift adopt failed")
        }
        _ = try OpaqueHandle(output)
        return OwnedSession(runtime: self, handle: output)
    }

    func consume(_ handle: UInt64) throws -> ConsumeEvidence {
        let result = try consumeRawOnOwner(handle: handle, length: ConsumeEvidence.byteCount)
        if result.0.isSuccess { return result.1 }
        throw NativeCallFailure(status: result.0, evidence: result.1)
    }

    func consumeRawOnOwner(handle: UInt64, length: UInt32) throws -> (StatusWord, ConsumeEvidence) {
        let call: (StatusWord, ConsumeEvidence) = try fifo.call {
            var native = poisonedEvidence()
            let status = StatusWord(raw: spx_private_apple_swift_v1_consume(handle, &native, length))
            return (status, ConsumeEvidence(words: evidenceWords(&native)))
        }
        let status = call.0
        try status.requireWellFormed()
        return call
    }

    func consumeRawWrongThread(handle: UInt64) throws -> (StatusWord, ConsumeEvidence) {
        var native = poisonedEvidence()
        let status = StatusWord(raw: spx_private_apple_swift_v1_consume(
            handle, &native, ConsumeEvidence.byteCount))
        try status.requireWellFormed()
        return (status, ConsumeEvidence(words: evidenceWords(&native)))
    }

    func adoptSingleWitness() throws -> OpaqueHandle {
        let call: (StatusWord, UInt64) = try fifo.call {
            var output: UInt64 = 0
            let status = StatusWord(raw: spx_private_apple_swift_v1_adopt_single(
                RequiresFalseEvidence.finalizerPayloadKAT, &output))
            return (status, output)
        }
        let status = call.0
        let output = call.1
        try status.requireWellFormed()
        if !status.isSuccess {
            try requireContract(output == 0, "witness adoption rejection modified output handle")
            throw ContractFailure(description: "private Swift witness adoption failed")
        }
        return try OpaqueHandle(output)
    }

    func executeRequiresFalse(_ handle: UInt64) throws -> RequiresFalseEvidence {
        let result = try executeRequiresFalseOnOwner(
            handle: handle, length: RequiresFalseEvidence.byteCount)
        try requireContract(result.0.isSuccess, "private Swift requires-false witness failed")
        return result.1
    }

    func executeRequiresFalseOnOwner(
        handle: UInt64, length: UInt32) throws -> (StatusWord, RequiresFalseEvidence) {
        let call: (StatusWord, RequiresFalseEvidence) = try fifo.call {
            var native = poisonedEvidence()
            let status = StatusWord(raw: spx_private_apple_swift_v1_execute_requires_false(
                handle, &native, length))
            return (status, RequiresFalseEvidence(words: evidenceWords(&native)))
        }
        let status = call.0
        try status.requireWellFormed()
        return call
    }

    func closeRaw() throws -> StatusWord {
        StatusWord(raw: try fifo.call { spx_private_apple_swift_v1_close_runtime() })
    }

    func close() throws {
        try barrier()
        let status = try closeRaw()
        try status.requireWellFormed()
        try requireContract(status.isSuccess, "private Swift runtime close failed")
        try fifo.shutdown()
    }

    func barrier() throws { try fifo.barrier() }

    func enqueueCleanup(_ handle: UInt64) {
        do {
            try fifo.enqueue { [self] in
                let result: Result<ConsumeEvidence, Error>
                do { result = .success(try consumeDirect(handle)) }
                catch { result = .failure(error) }
                cleanupLock.lock()
                cleanupResults.append(result)
                cleanupLock.unlock()
            }
        } catch {
            // ARC fallback is deliberately nonthrowing and never retries rejected work.
        }
    }

    func takeCleanupResult() throws -> ConsumeEvidence {
        try barrier()
        cleanupLock.lock()
        let results = cleanupResults
        cleanupResults.removeAll()
        cleanupLock.unlock()
        try requireContract(results.count == 1, "cleanup did not consume exactly once")
        return try results[0].get()
    }

    func takeCleanupResultOrNil() throws -> ConsumeEvidence? {
        try barrier()
        cleanupLock.lock()
        let results = cleanupResults
        cleanupResults.removeAll()
        cleanupLock.unlock()
        try requireContract(results.count <= 1, "cleanup consumed more than once")
        return try results.first?.get()
    }

    private func consumeDirect(_ handle: UInt64) throws -> ConsumeEvidence {
        var native = poisonedEvidence()
        let status = StatusWord(raw: spx_private_apple_swift_v1_consume(
            handle, &native, ConsumeEvidence.byteCount))
        let evidence = ConsumeEvidence(words: evidenceWords(&native))
        if status.isSuccess { return evidence }
        throw NativeCallFailure(status: status, evidence: evidence)
    }
}
