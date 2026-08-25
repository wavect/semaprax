package dev.semaprax.runtime

import android.os.Handler
import android.os.HandlerThread
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicReference

internal class NativeRuntime(private val bridge: NativeBridge) : AutoCloseable {
    private val thread = HandlerThread("semaprax-private-runtime").apply { start() }
    private val handler = Handler(thread.looper)
    private val cleanerObservation = AtomicReference<ConsumeResult?>(null)
    private var closed = false

    fun open(provider: java.io.File) {
        open(provider, NativeBridge.SELECTOR_DISCARD)
    }

    fun open(
        provider: java.io.File,
        selector: Int,
    ) {
        require(call { bridge.open(provider, selector) }.isSuccess) { "native runtime open failed" }
    }

    fun adopt(): OwnedSession {
        val adopted = call { bridge.adoptPair() }
        require(adopted.status.isSuccess) { "native adoption failed" }
        OpaqueHandle.decode(adopted.handle)
        return OwnedSession(this, adopted.handle)
    }

    fun adoptSingleWitness(): Long {
        val adopted = call { bridge.adoptSingle() }
        require(adopted.status.isSuccess) { "native single-owner adoption failed" }
        OpaqueHandle.decode(adopted.handle)
        return adopted.handle
    }

    fun adoptOwnedWitness(): Long {
        val adopted = call { bridge.adoptOwned() }
        require(adopted.status.isSuccess) { "native owned-result adoption failed" }
        OpaqueHandle.decode(adopted.handle)
        return adopted.handle
    }

    fun executeRequiresFalse(handle: Long): ConsumeResult = call { bridge.executeRequiresFalse(handle) }

    fun executeIdentityMax(handle: Long): ConsumeResult = call { bridge.executeIdentityMax(handle) }

    fun consume(handle: Long): ConsumeResult = call { bridge.consume(handle) }

    fun probe(callback: Runnable): StatusWord = call { bridge.probeException(callback) }

    fun consumeWrongThread(handle: Long): ConsumeResult = bridge.consumeRawWrongThread(handle)

    fun enqueueCleaner(handle: Long) {
        if (!handler.post {
                try {
                    cleanerObservation.set(bridge.consume(handle))
                } catch (_: Throwable) {
                    // The deterministic barrier below observes missing evidence.
                }
            }
        ) {
            cleanerObservation.set(null)
        }
    }

    fun takeCleanerEvidence(): ConsumeResult {
        barrier()
        return cleanerObservation.getAndSet(null)
            ?: throw AssertionError("Cleaner did not publish native evidence")
    }

    fun takeCleanerEvidenceOrNull(): ConsumeResult? {
        barrier()
        return cleanerObservation.getAndSet(null)
    }

    fun barrier() {
        call { Unit }
    }

    override fun close() {
        if (closed) return
        // Drain every cleanup command already accepted by the owning FIFO
        // before enqueueing the thread-affine native runtime close.
        barrier()
        val status = call { bridge.closeRuntime() }
        require(status.isSuccess) { "native runtime close failed" }
        closed = true
        thread.quitSafely()
        if (Thread.currentThread() !== thread) thread.join()
    }

    internal fun <T> call(operation: () -> T): T {
        check(!closed) { "native runtime is closed" }
        if (Thread.currentThread() === thread) return operation()
        val result = AtomicReference<Result<T>>()
        val finished = CountDownLatch(1)
        check(handler.post {
            result.set(runCatching(operation))
            finished.countDown()
        }) { "native runtime queue rejected work" }
        var interrupted = false
        while (true) {
            try {
                finished.await()
                break
            } catch (_: InterruptedException) {
                // Accepted ownership work cannot be abandoned. Preserve the
                // signal only after the exact native result is recovered.
                interrupted = true
            }
        }
        if (interrupted) Thread.currentThread().interrupt()
        return result.get().getOrThrow()
    }
}
