package dev.semaprax.instrumentation

import android.app.Instrumentation
import android.app.Activity
import android.os.Bundle
import dev.semaprax.runtime.DeclaredFixtureException
import dev.semaprax.runtime.NativeBridge
import dev.semaprax.runtime.NativeRuntime
import dev.semaprax.runtime.OpaqueHandle
import dev.semaprax.runtime.OwnedSession
import dev.semaprax.runtime.StatusWord
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicReference

class ContractInstrumentation : Instrumentation() {
    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
        start()
    }

    override fun onStart() {
        val output = File(targetContext.filesDir, RESULT_FILE)
        val result = runCatching { verifyExactContract() }
        if (result.isSuccess) {
            output.writeText(EXPECTED_RESULT, Charsets.UTF_8)
            finish(Activity.RESULT_OK, Bundle().apply { putString("semaprax", "pass") })
        } else {
            output.writeText("SEMAPRAX_ANDROID_JNI_V1_FAIL\n", Charsets.UTF_8)
            finish(Activity.RESULT_CANCELED, Bundle().apply { putString("semaprax", "fail") })
        }
    }

    private fun verifyExactContract() {
        OpaqueHandle.requireKnownAnswer()
        StatusWord.requireKnownAnswers()
        require(android.os.Build.VERSION.SDK_INT == 35) { "emulator API changed" }
        require(android.os.Build.SUPPORTED_ABIS.firstOrNull() == "x86_64") { "emulator ABI changed" }
        val nativeDirectory = File(targetContext.applicationInfo.nativeLibraryDir).canonicalFile
        val bridge = NativeBridge.loadExact(nativeDirectory)

        runProvider(bridge, nativeDirectory, "libsemaprax_provider_o0.so", cleaner = false)
        runProvider(bridge, nativeDirectory, "libsemaprax_provider_o2.so", cleaner = true)
        runConsumeCleanerRace(bridge, nativeDirectory)
    }

    private fun runProvider(
        bridge: NativeBridge,
        nativeDirectory: File,
        providerName: String,
        cleaner: Boolean,
    ) {
        val provider = File(nativeDirectory, providerName).canonicalFile
        require(provider.parentFile == nativeDirectory && provider.isFile) {
            "provider is not the exact installed image"
        }
        val runtime = NativeRuntime(bridge)
        runtime.open(provider)
        val session = runtime.adopt()
        val handle = session.currentHandle
        if (!cleaner) {
            require(handle == NativeBridge.HANDLE_KAT) {
                "first observed native handle does not match the frozen known answer"
            }
        }

        if (!cleaner) {
            val crossRuntime = NativeRuntime(bridge)
            crossRuntime.open(provider)
            crossRuntime.call { bridge.consume(handle) }.also {
                require(it.status.raw == StatusWord.KAT_CROSS_RUNTIME)
                it.requireUntouchedFailure()
            }
            crossRuntime.close()
        }

        val forged = runtime.call { bridge.consume(handle xor 1L) }
        require(forged.status.raw == StatusWord.KAT_INVALID_HANDLE)
        forged.requireUntouchedFailure()
        val wrongThread = runtime.consumeWrongThread(handle)
        require(wrongThread.status.raw == StatusWord.KAT_WRONG_THREAD)
        wrongThread.requireUntouchedFailure()

        if (cleaner) {
            session.cleanForTest()
            runtime.takeCleanerEvidence().requireExact()
        } else {
            session.consume().requireExact()
        }

        val stale = runtime.call { bridge.consume(handle) }
        require(stale.status.raw == StatusWord.KAT_STALE_HANDLE)
        stale.requireUntouchedFailure()

        // A second exact call proves provider trace reset is per-consume, not
        // merely per-open.
        runtime.adopt().consume().requireExact()
        runInterruptedAcceptedWork(runtime)

        require(runtime.probe(Runnable {}).isSuccess)
        require(
            runtime.probe(Runnable {
                runtime.call { bridge.adoptPair() }
                    .requireUntouchedFailure(StatusWord.KAT_REENTRANT)
            }).isSuccess,
        )
        require(
            runtime.probe(Runnable { throw DeclaredFixtureException() }).raw ==
                StatusWord.KAT_DECLARED_FIXTURE,
        )
        require(
            runtime.probe(Runnable { throw IllegalStateException() }).raw ==
                StatusWord.KAT_UNEXPECTED_ADAPTER,
        )
        runtime.close()
    }

    private fun runInterruptedAcceptedWork(runtime: NativeRuntime) {
        val adopted = AtomicReference<OwnedSession?>(null)
        val adopter = Thread({
            Thread.currentThread().interrupt()
            adopted.set(runtime.adopt())
            require(Thread.interrupted()) { "accepted adopt did not restore interrupt status" }
        }, "semaprax-interrupted-adopt")
        adopter.start()
        adopter.join()

        val consumed = AtomicReference<dev.semaprax.runtime.ConsumeResult?>(null)
        val consumer = Thread({
            Thread.currentThread().interrupt()
            consumed.set(checkNotNull(adopted.get()).consume())
            require(Thread.interrupted()) { "accepted consume did not restore interrupt status" }
        }, "semaprax-interrupted-consume")
        consumer.start()
        consumer.join()
        checkNotNull(consumed.get()).requireExact()
    }

    private fun runConsumeCleanerRace(bridge: NativeBridge, nativeDirectory: File) {
        val provider = File(nativeDirectory, "libsemaprax_provider_o2.so").canonicalFile
        val runtime = NativeRuntime(bridge)
        runtime.open(provider)
        val session = runtime.adopt()
        val start = CountDownLatch(1)
        val explicit = AtomicReference<dev.semaprax.runtime.ConsumeResult?>(null)
        val first = Thread({
            start.await()
            runCatching { session.consume() }.getOrNull()?.let { explicit.set(it) }
        }, "semaprax-explicit-consume-racer")
        val second = Thread({
            start.await()
            session.cleanForTest()
        }, "semaprax-cleaner-racer")
        first.start()
        second.start()
        start.countDown()
        first.join()
        second.join()
        val cleaned = runtime.takeCleanerEvidenceOrNull()
        val completed = listOfNotNull(explicit.get(), cleaned)
        require(completed.size == 1) { "consume-versus-cleaner did not consume exactly once" }
        completed.single().requireExact()
        session.close()
        runtime.close()
    }

    companion object {
        const val RESULT_FILE = "semaprax-android-jni-v1.txt"
        const val EXPECTED_RESULT =
            "SEMAPRAX_ANDROID_JNI_V1_OK api=35 abi=x86_64 o0=explicit o2=cleaner " +
                "handle=0001000001000001 declared=0000006b00000007 " +
                "unexpected=0000004500000001 finalizers=1:13,0:11 " +
                "publication=no-owned allocations=0 handles=0\n"
    }
}
