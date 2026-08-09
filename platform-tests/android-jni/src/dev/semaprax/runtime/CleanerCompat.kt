package dev.semaprax.runtime

import java.lang.ref.PhantomReference
import java.lang.ref.ReferenceQueue
import java.util.Collections
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean

/** API-28 Cleaner-compatible exactly-once phantom-reference dispatcher. */
internal class CleanerCompat private constructor() {
    private val queue = ReferenceQueue<Any>()
    private val live = Collections.newSetFromMap(ConcurrentHashMap<Entry, Boolean>())

    init {
        Thread({ drainForever() }, "semaprax-private-cleaner").apply {
            isDaemon = true
            start()
        }
    }

    fun register(owner: Any, action: Runnable): Cleanable {
        val entry = Entry(owner, queue, action) { live.remove(it) }
        live.add(entry)
        return entry
    }

    private fun drainForever() {
        while (true) {
            try {
                (queue.remove() as Entry).clean()
            } catch (_: InterruptedException) {
                // Interruption is not a semantic cancellation channel.
            } catch (_: Throwable) {
                // Automatic JVM cleanup is infallible and never reports status.
            }
        }
    }

    internal interface Cleanable {
        fun clean()
    }

    private class Entry(
        owner: Any,
        queue: ReferenceQueue<Any>,
        private val action: Runnable,
        private val remove: (Entry) -> Unit,
    ) : PhantomReference<Any>(owner, queue), Cleanable {
        private val completed = AtomicBoolean(false)

        override fun clean() {
            if (!completed.compareAndSet(false, true)) return
            remove(this)
            clear()
            try {
                action.run()
            } catch (_: Throwable) {
                // A Cleaner action cannot become application-visible failure.
            }
        }
    }

    companion object {
        val shared = CleanerCompat()
    }
}
