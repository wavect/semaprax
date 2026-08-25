package dev.semaprax.runtime

import java.io.File

internal class NativeBridge private constructor() {
    private external fun nativeOpen(providerPathUtf8: ByteArray, selector: Int): Long
    private external fun nativeAdoptPair(
        firstPayload: Long,
        secondPayload: Long,
        outHandle: LongArray,
    ): Long

    private external fun nativeAdoptSingle(payload: Long, outHandle: LongArray): Long
    private external fun nativeAdoptOwned(payload: Long, outHandle: LongArray): Long
    private external fun nativeConsume(handle: Long, outEvidence: LongArray): Long
    private external fun nativeExecuteRequiresFalse(handle: Long, outEvidence: LongArray): Long
    private external fun nativeExecuteIdentityMax(handle: Long, outEvidence: LongArray): Long
    private external fun nativeCloseRuntime(): Long
    private external fun nativeProbeException(callback: Runnable): Long
    private external fun nativeConsumeRawWrongThread(handle: Long, outEvidence: LongArray): Long

    fun open(
        provider: File,
        selector: Int = SELECTOR_DISCARD,
    ): StatusWord = StatusWord.decode(nativeOpen(provider.path.toByteArray(Charsets.UTF_8), selector))

    fun adoptPair(firstPayload: Long = 11L, secondPayload: Long = 13L): AdoptResult {
        val output = longArrayOf(POISON)
        val status = StatusWord.decode(nativeAdoptPair(firstPayload, secondPayload, output))
        return AdoptResult(status, output[0])
    }

    fun adoptSingle(payload: Long = REQUIRE_FALSE_OWNER_PAYLOAD): AdoptResult {
        val output = longArrayOf(POISON)
        val status = StatusWord.decode(nativeAdoptSingle(payload, output))
        return AdoptResult(status, output[0])
    }

    fun adoptOwned(payload: Long = IDENTITY_MAX_OWNER_PAYLOAD): AdoptResult {
        val output = longArrayOf(POISON)
        val status = StatusWord.decode(nativeAdoptOwned(payload, output))
        return AdoptResult(status, output[0])
    }

    fun consume(handle: Long): ConsumeResult {
        val output = LongArray(EVIDENCE_WORDS) { POISON }
        val status = StatusWord.decode(nativeConsume(handle, output))
        return ConsumeResult(status, output)
    }

    fun executeRequiresFalse(handle: Long): ConsumeResult {
        val output = LongArray(EVIDENCE_WORDS) { POISON }
        val status = StatusWord.decode(nativeExecuteRequiresFalse(handle, output))
        return ConsumeResult(status, output)
    }

    fun executeIdentityMax(handle: Long): ConsumeResult {
        val output = LongArray(EVIDENCE_WORDS) { POISON }
        val status = StatusWord.decode(nativeExecuteIdentityMax(handle, output))
        return ConsumeResult(status, output)
    }

    fun consumeRawWrongThread(handle: Long): ConsumeResult {
        val output = LongArray(EVIDENCE_WORDS) { POISON }
        val status = StatusWord.decode(nativeConsumeRawWrongThread(handle, output))
        return ConsumeResult(status, output)
    }

    fun closeRuntime(): StatusWord = StatusWord.decode(nativeCloseRuntime())

    fun probeException(callback: Runnable): StatusWord =
        StatusWord.decode(nativeProbeException(callback))

    companion object {
        const val HANDLE_KAT = 0x0001000001000001L
        const val EVIDENCE_WORDS = 8
        const val EVIDENCE_VERSION = 1L
        const val EVIDENCE_PROOF_MASK = 0x0fL
        const val EXPECTED_FIRST_FINALIZER = (1L shl 32) or 13L
        const val EXPECTED_SECOND_FINALIZER = 11L
        const val SELECTOR_DISCARD = 0
        const val SELECTOR_REQUIRES_FALSE = 1
        const val SELECTOR_IDENTITY_MAX = 2
        const val REQUIRE_FALSE_OWNER_PAYLOAD = -1L
        const val REQUIRE_FALSE_STATUS_WORD = 1L
        const val REQUIRE_FALSE_FINALIZER_COUNT = 1L
        const val REQUIRE_FALSE_FINALIZER =
            (0L shl 32) or REQUIRE_FALSE_OWNER_PAYLOAD
        const val IDENTITY_MAX_OWNER_PAYLOAD = -1L
        const val IDENTITY_MAX_PUBLICATIONS = 2L
        const val POISON = -0x3501450135014502L

        fun loadExact(nativeLibraryDirectory: File): NativeBridge {
            val directory = nativeLibraryDirectory.canonicalFile
            require(directory.isDirectory) { "native library directory is unavailable" }
            val library = File(directory, "libsemaprax_jni.so").canonicalFile
            require(library.parentFile == directory && library.isFile) {
                "JNI library is not the exact installed image"
            }
            System.load(library.path)
            return NativeBridge()
        }
    }
}

internal data class AdoptResult(val status: StatusWord, val handle: Long) {
    fun requireUntouchedFailure(expected: Long) {
        require(status.raw == expected) { "adoption failure status is not exact" }
        require(handle == NativeBridge.POISON) { "failed adoption mutated caller output" }
    }
}

internal data class ConsumeResult(val status: StatusWord, val evidence: LongArray) {
    fun requireExact() {
        require(status.isSuccess) { "consume status is nonzero" }
        require(evidence.size == NativeBridge.EVIDENCE_WORDS)
        require(evidence[0] == NativeBridge.EVIDENCE_VERSION)
        require(evidence[1] > 0L) { "module instance identity is zero" }
        require(evidence[2] == NativeBridge.EVIDENCE_PROOF_MASK)
        require(evidence[3] == 0L) { "postcommit allocation count is nonzero" }
        require(evidence[4] == 2L) { "physical finalizer count is not exact" }
        require(evidence[5] == NativeBridge.EXPECTED_FIRST_FINALIZER)
        require(evidence[6] == NativeBridge.EXPECTED_SECOND_FINALIZER)
        require(evidence[7] == 0L) { "native host state is unhealthy" }
    }

    fun requireRequiresFalseExact() {
        require(status.isSuccess) { "requires-false witness status is nonzero" }
        require(evidence.size == NativeBridge.EVIDENCE_WORDS)
        require(evidence[0] == NativeBridge.EVIDENCE_VERSION)
        require(evidence[1] > 0L) { "module instance identity is zero" }
        require(evidence[2] == NativeBridge.REQUIRE_FALSE_STATUS_WORD) {
            "semantic failure selection word is not the canonical requires ordinal"
        }
        require(evidence[3] == 0L) { "postcommit allocation count is nonzero" }
        require(evidence[4] == NativeBridge.REQUIRE_FALSE_FINALIZER_COUNT) {
            "physical finalizer count is not exactly one"
        }
        require(evidence[5] == NativeBridge.REQUIRE_FALSE_FINALIZER) {
            "witness finalizer owner and payload are not the canonical corpus values"
        }
        require(evidence[6] == 0L) { "witness mutated a second owner slot" }
        require(evidence[7] == 0L) { "native host state is unhealthy" }
    }

    fun requireIdentityMaxExact() {
        require(status.isSuccess) { "identity-max witness status is nonzero" }
        require(evidence.size == NativeBridge.EVIDENCE_WORDS)
        require(evidence[0] == NativeBridge.EVIDENCE_VERSION)
        require(evidence[1] > 0L) { "module instance identity is zero" }
        require(evidence[2] == NativeBridge.IDENTITY_MAX_PUBLICATIONS) {
            "outward publication word is not the canonical identity-max count"
        }
        require(evidence[3] == 0L) { "postcommit allocation count is nonzero" }
        require(evidence[4] == 0L) { "published identity was physically finalized" }
        require(evidence[5] == 0L && evidence[6] == 0L) {
            "publication mutated a finalizer slot"
        }
        require(evidence[7] == 0L) { "native host state is unhealthy" }
    }

    fun requireUntouchedFailure() {
        require(!status.isSuccess) { "hostile consume unexpectedly succeeded" }
        require(evidence.all { it == NativeBridge.POISON }) {
            "failed consume mutated caller evidence"
        }
    }
}
