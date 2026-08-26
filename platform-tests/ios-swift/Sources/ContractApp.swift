import Foundation
import UIKit

private final class StartGate: @unchecked Sendable {
    private let condition = NSCondition()
    private var open = false
    func await() {
        condition.lock(); while !open { condition.wait() }; condition.unlock()
    }
    func release() {
        condition.lock(); open = true; condition.broadcast(); condition.unlock()
    }
}

private final class LockedBox<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Value
    init(_ value: Value) { self.value = value }
    func set(_ newValue: Value) { lock.lock(); value = newValue; lock.unlock() }
    func get() -> Value { lock.lock(); defer { lock.unlock() }; return value }
}

enum ContractRunner {
    static let resultFile = "semaprax-ios-swift-v1.txt"

    #if SEMAPRAX_EXPLICIT
    static let mode = "explicit"
    static let optimization = "O0"
    static let expected = "SEMAPRAX_IOS_SWIFT_V1_OK mode=explicit optimization=O0 target=arm64-simulator handle=0001000001000001 wrong-thread=0000002d00000002 invalid=0000002d00000007 stale=0000002d00000008 finalizers=1:13,0:11 publication=no-owned allocations=0 handles=0 rf=1 om=1\n"
    #elseif SEMAPRAX_DEINIT
    static let mode = "deinit"
    static let optimization = "O2"
    static let expected = "SEMAPRAX_IOS_SWIFT_V1_OK mode=deinit optimization=O2 target=arm64-simulator handle=0001000001000001 wrong-thread=0000002d00000002 invalid=0000002d00000007 stale=0000002d00000008 finalizers=1:13,0:11 publication=no-owned allocations=0 handles=0 rf=1 om=1\n"
    #else
    #error("the private Swift app requires an exact evidence mode")
    #endif

    static func run() throws {
        try StatusWord.requireKnownAnswers()
        try OpaqueHandle.requireKnownAnswer()
        let runtime = NativeRuntime()
        try runtime.open()
        let alreadyOpen = try runtime.openAgainRaw()
        try requireContract(alreadyOpen.raw == StatusWord.alreadyOpenKAT, "already-open KAT changed")

        #if SEMAPRAX_EXPLICIT
        let session = try runtime.adopt()
        try requireContract(session.currentHandle == OpaqueHandle.knownAnswer, "first handle KAT changed")
        try adversarialPrecommit(runtime: runtime, session: session)
        try session.consume().requireExact()
        try assertStale(runtime: runtime, handle: OpaqueHandle.knownAnswer)
        #else
        weak var released: OwnedSession?
        var handle: UInt64 = 0
        do {
            var session: OwnedSession? = try runtime.adopt()
            handle = session!.currentHandle
            released = session
            try requireContract(handle == OpaqueHandle.knownAnswer, "first handle KAT changed")
            try adversarialPrecommit(runtime: runtime, session: session!)
            session = nil
        }
        try runtime.takeCleanupResult().requireExact()
        try requireContract(released == nil, "ARC deinit cleanup was not deterministic")
        try assertStale(runtime: runtime, handle: handle)
        #endif

        // A second call on the same target-bound runtime proves per-consume trace reset.
        try runtime.adopt().consume().requireExact()
        try assertLiveClose(runtime: runtime)
        try runConsumeDeinitRace(runtime: runtime)
        try runtime.close()
        try runRequiresFalseWitness()
        try runIdentityMaxWitness()
    }

    private static func runRequiresFalseWitness() throws {
        let runtime = NativeRuntime()
        try runtime.openRequiresFalse()
        let alreadyOpen = try runtime.openRequiresFalseAgainRaw()
        try requireContract(alreadyOpen.raw == StatusWord.alreadyOpenKAT,
                            "witness already-open KAT changed")

        let witness = try runtime.adoptSingleWitness()

        // The pair-consume lane cannot drive a witness session.
        let wrongShape = try runtime.consumeRawOnOwner(
            handle: witness.raw, length: ConsumeEvidence.byteCount)
        try requireContract(wrongShape.0.raw == StatusWord.invalidHandleKAT,
                            "witness-shape consume was not rejected")
        try wrongShape.1.requirePoisoned()

        let forged = try runtime.executeRequiresFalseOnOwner(
            handle: witness.raw ^ 1, length: RequiresFalseEvidence.byteCount)
        try requireContract(forged.0.raw == StatusWord.invalidHandleKAT,
                            "forged witness handle KAT changed")
        try forged.1.requirePoisoned()

        // The canonical requires-false corpus witness: one adopted owner at the
        // maximum payload fails the `requires allowed` guard, publishes no owned
        // result, and finalizes exactly that one owner after selection.
        try runtime.executeRequiresFalse(witness.raw).requireExact()

        // Failure selection is sticky: the consumed owner cannot retry.
        let stale = try runtime.executeRequiresFalseOnOwner(
            handle: witness.raw, length: RequiresFalseEvidence.byteCount)
        try requireContract(stale.0.raw == StatusWord.staleHandleKAT,
                            "stale witness handle KAT changed")
        try stale.1.requirePoisoned()
        try runtime.close()
    }

    private static func runIdentityMaxWitness() throws {
        let runtime = NativeRuntime()
        try runtime.openIdentityMax()
        let alreadyOpen = try runtime.openIdentityMaxAgainRaw()
        try requireContract(alreadyOpen.raw == StatusWord.alreadyOpenKAT,
                            "owned-result already-open KAT changed")

        let witness = try runtime.adoptOwnedWitness()

        // The pair-consume lane cannot consume inside the owned-result image.
        let wrongShape = try runtime.consumeRawOnOwner(
            handle: witness.raw, length: ConsumeEvidence.byteCount)
        try requireContract(wrongShape.0.raw == StatusWord.invalidHandleKAT,
                            "owned-result-shape consume was not rejected")
        try wrongShape.1.requirePoisoned()

        let forged = try runtime.executeIdentityMaxOnOwner(
            handle: witness.raw ^ 1, length: IdentityMaxEvidence.byteCount)
        try requireContract(forged.0.raw == StatusWord.invalidHandleKAT,
                            "forged owned-result handle KAT changed")
        try forged.1.requirePoisoned()

        // The canonical identity-max corpus witness: one adopted owner at the
        // maximum payload is published outward as the owned result without any
        // physical finalization; the pre-publication generation becomes stale
        // and the refreshed published owner re-adopts for exactly one more
        // publication.
        try runtime.executeIdentityMax(witness.raw).requireExact()

        // Outward publication rotated the consumed generation: the stale handle
        // cannot retry without poisoning the host.
        let stale = try runtime.executeIdentityMaxOnOwner(
            handle: witness.raw, length: IdentityMaxEvidence.byteCount)
        try requireContract(stale.0.raw == StatusWord.staleHandleKAT,
                            "stale owned-result handle KAT changed")
        try stale.1.requirePoisoned()
        try runtime.close()
    }

    private static func adversarialPrecommit(runtime: NativeRuntime, session: OwnedSession) throws {
        let handle = session.currentHandle
        let forged = try runtime.consumeRawOnOwner(handle: handle ^ 1, length: ConsumeEvidence.byteCount)
        try requireContract(forged.0.raw == StatusWord.invalidHandleKAT, "forged-handle KAT changed")
        try forged.1.requirePoisoned()
        let wrongThread = try runtime.consumeRawWrongThread(handle: handle)
        try requireContract(wrongThread.0.raw == StatusWord.wrongThreadKAT, "wrong-thread KAT changed")
        try wrongThread.1.requirePoisoned()
        let wrongLength = try runtime.consumeRawOnOwner(handle: handle, length: 63)
        try requireContract(wrongLength.0.raw == StatusWord.adapterKAT, "invalid-length KAT changed")
        try wrongLength.1.requirePoisoned()
        try requireContract(session.currentHandle == handle, "precommit rejection consumed ownership")
    }

    private static func assertStale(runtime: NativeRuntime, handle: UInt64) throws {
        let stale = try runtime.consumeRawOnOwner(handle: handle, length: ConsumeEvidence.byteCount)
        try requireContract(stale.0.raw == StatusWord.staleHandleKAT, "stale-handle KAT changed")
        try stale.1.requirePoisoned()
        let noRetry = try runtime.consumeRawOnOwner(handle: handle, length: ConsumeEvidence.byteCount)
        try requireContract(noRetry.0.raw == StatusWord.staleHandleKAT, "consumed handle became retryable")
        try noRetry.1.requirePoisoned()
    }

    private static func assertLiveClose(runtime: NativeRuntime) throws {
        let session = try runtime.adopt()
        let live = try runtime.closeRaw()
        try requireContract(live.raw == StatusWord.liveSessionsKAT, "live-session close KAT changed")
        try requireContract(session.currentHandle != 0, "rejected close consumed a live session")
        try session.consume().requireExact()
    }

    private static func runConsumeDeinitRace(runtime: NativeRuntime) throws {
        let session = try runtime.adopt()
        let handle = session.currentHandle
        let gate = StartGate()
        let explicit = LockedBox<Result<ConsumeEvidence, Error>?>(nil)
        let explicitThread = Thread {
            gate.await()
            explicit.set(Result { try session.consume() })
        }
        explicitThread.name = "semaprax-explicit-racer"
        let cleanupThread = Thread {
            gate.await()
            try? session.cleanForTest()
        }
        cleanupThread.name = "semaprax-deinit-action-racer"
        explicitThread.start()
        cleanupThread.start()
        gate.release()
        while !explicitThread.isFinished || !cleanupThread.isFinished { Thread.sleep(forTimeInterval: 0.001) }
        let cleaned = try runtime.takeCleanupResultOrNil()
        let explicitResult = explicit.get()
        let explicitEvidence = try? explicitResult?.get()
        let completed = [explicitEvidence, cleaned].compactMap { $0 }
        try requireContract(completed.count == 1, "consume-versus-deinit action did not consume exactly once")
        try completed[0].requireExact()
        try assertStale(runtime: runtime, handle: handle)
        withExtendedLifetime(session) {}
    }
}

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication,
                     didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        let worker = Thread {
            let marker: String
            do {
                try ContractRunner.run()
                marker = ContractRunner.expected
            } catch {
                marker = "SEMAPRAX_IOS_SWIFT_V1_FAIL\n"
            }
            do {
                let directory = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                try Data(marker.utf8).write(to: directory.appendingPathComponent(ContractRunner.resultFile), options: .atomic)
            } catch {
                // The hosted gate fails closed when the exact app-private result is absent.
            }
        }
        worker.name = "semaprax-private-swift-contract"
        worker.start()
        return true
    }
}
