import Foundation

final class OwnedSession: @unchecked Sendable {
    private final class Cell: @unchecked Sendable {
        private let lock = NSLock()
        private weak var runtime: NativeRuntime?
        private var handle: UInt64
        private var inFlight = false
        private var cleanupRequested = false

        init(runtime: NativeRuntime, handle: UInt64) {
            self.runtime = runtime
            self.handle = handle
        }

        func peek() -> UInt64 {
            lock.lock(); defer { lock.unlock() }
            return handle
        }

        func beginConsume() -> UInt64? {
            lock.lock(); defer { lock.unlock() }
            guard handle != 0 && !inFlight else { return nil }
            inFlight = true
            return handle
        }

        func finish(_ owned: UInt64, status: StatusWord) {
            var deferredCleanup: UInt64?
            lock.lock()
            precondition(inFlight && handle == owned)
            inFlight = false
            if status.isPrecommitAppleRejection {
                if cleanupRequested {
                    cleanupRequested = false
                    handle = 0
                    deferredCleanup = owned
                }
            } else {
                cleanupRequested = false
                handle = 0
            }
            lock.unlock()
            if let deferredCleanup { runtime?.enqueueCleanup(deferredCleanup) }
        }

        func requestCleanup() {
            var claimed: UInt64?
            lock.lock()
            if inFlight {
                cleanupRequested = true
            } else if handle != 0 {
                claimed = handle
                handle = 0
            }
            lock.unlock()
            if let claimed { runtime?.enqueueCleanup(claimed) }
        }
    }

    private let runtime: NativeRuntime
    private let cell: Cell

    init(runtime: NativeRuntime, handle: UInt64) {
        self.runtime = runtime
        self.cell = Cell(runtime: runtime, handle: handle)
    }

    var currentHandle: UInt64 { cell.peek() }

    func consume() throws -> ConsumeEvidence {
        guard let handle = cell.beginConsume() else {
            throw ContractFailure(description: "owned session is already consumed or in flight")
        }
        do {
            let evidence = try runtime.consume(handle)
            cell.finish(handle, status: StatusWord(raw: 0))
            withExtendedLifetime(self) {}
            return evidence
        } catch let failure as NativeCallFailure {
            cell.finish(handle, status: failure.status)
            withExtendedLifetime(self) {}
            throw failure
        } catch {
            // An unclassified host-side failure is terminal; ownership cannot be retried safely.
            cell.finish(handle, status: StatusWord(raw: StatusWord.uncertaintyKAT))
            withExtendedLifetime(self) {}
            throw error
        }
    }

    func cleanForTest() throws {
        cell.requestCleanup() // Exercises the identical action used by deinit.
        try runtime.barrier()
    }

    deinit {
        cell.requestCleanup()
    }
}
