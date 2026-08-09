package dev.semaprax.runtime

import java.lang.ref.Reference

internal class OwnedSession(
    private val runtime: NativeRuntime,
    handle: Long,
) : AutoCloseable {
    private val state = CleanupState(runtime, handle)
    private val cleanable = CleanerCompat.shared.register(this, state)

    val currentHandle: Long
        get() = state.peek()

    fun consume(): ConsumeResult {
        val handle = state.beginConsume()
        require(handle != 0L) { "owned session is already consumed" }
        val result = runtime.consume(handle)
        Reference.reachabilityFence(this)
        if (result.status.isPrecommitAndroidRejection()) {
            state.finishPrecommit(handle)?.let(runtime::enqueueCleaner)
            return result
        }
        state.finishTerminal(handle)
        cleanable.clean()
        return result
    }

    fun cleanForTest() {
        cleanable.clean()
        runtime.barrier()
    }

    override fun close() {
        cleanable.clean()
    }

    private class CleanupState(
        private val runtime: NativeRuntime,
        initialHandle: Long,
    ) : Runnable {
        private var handle = initialHandle
        private var inFlight = false
        private var cleanupRequested = false

        @Synchronized
        fun peek(): Long = handle

        @Synchronized
        fun beginConsume(): Long {
            if (handle == 0L || inFlight) return 0L
            inFlight = true
            return handle
        }

        @Synchronized
        fun finishPrecommit(owned: Long): Long? {
            check(inFlight && handle == owned) { "precommit ownership state mismatch" }
            inFlight = false
            if (!cleanupRequested) return null
            cleanupRequested = false
            handle = 0L
            return owned
        }

        @Synchronized
        fun finishTerminal(owned: Long) {
            check(inFlight && handle == owned) { "terminal ownership state mismatch" }
            inFlight = false
            cleanupRequested = false
            handle = 0L
        }

        override fun run() {
            val owned = synchronized(this) {
                if (inFlight) {
                    cleanupRequested = true
                    return
                }
                val claimed = handle
                handle = 0L
                claimed
            }
            if (owned == 0L) return
            try {
                runtime.enqueueCleaner(owned)
            } catch (_: Throwable) {
                // Cleaner fallback is deliberately non-throwing.
            }
        }
    }
}
