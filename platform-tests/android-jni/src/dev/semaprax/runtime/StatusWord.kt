package dev.semaprax.runtime

internal data class StatusWord(
    val raw: Long,
    val code: Long,
    val statusClass: Int,
    val retry: Int,
    val domain: Int,
) {
    val isSuccess: Boolean
        get() = raw == SUCCESS

    init {
        require(raw >= 0L) { "SPXAJS01 reserved high bits are nonzero" }
        require(raw ushr RESERVED_SHIFT == 0L) { "SPXAJS01 reserved bits are nonzero" }
        if (raw == SUCCESS) {
            require(code == 0L && statusClass == 0 && retry == 0 && domain == 0)
        } else {
            require(code != 0L) { "SPXAJS01 failure code is zero" }
            require(statusClass in 1..5) { "SPXAJS01 status class is invalid" }
            require(retry in 0..2) { "SPXAJS01 retryability is invalid" }
            require(domain in DOMAIN_ANDROID..DOMAIN_FIXTURE) { "SPXAJS01 domain is invalid" }
        }
    }

    companion object {
        const val SUCCESS = 0L
        const val KAT_ANDROID_ADAPTER = 0x0000002d00000001L
        const val KAT_WRONG_THREAD = 0x0000002d00000002L
        const val KAT_INVALID_HANDLE = 0x0000002d00000007L
        const val KAT_STALE_HANDLE = 0x0000002d00000008L
        const val KAT_CROSS_RUNTIME = 0x0000002d00000009L
        const val KAT_REENTRANT = 0x0000002d0000000bL
        const val KAT_DECLARED_FIXTURE = 0x0000006b00000007L
        const val KAT_UNEXPECTED_ADAPTER = 0x0000004500000001L

        const val DOMAIN_ANDROID = 1
        const val DOMAIN_UNEXPECTED = 2
        const val DOMAIN_FIXTURE = 3

        private const val CLASS_SHIFT = 32
        private const val RETRY_SHIFT = 35
        private const val DOMAIN_SHIFT = 37
        private const val RESERVED_SHIFT = 53

        fun encode(code: Long, statusClass: Int, retry: Int, domain: Int): Long {
            require(code in 1L..0xffff_ffffL)
            require(statusClass in 1..5)
            require(retry in 0..2)
            require(domain in 1..0xffff)
            return code or
                (statusClass.toLong() shl CLASS_SHIFT) or
                (retry.toLong() shl RETRY_SHIFT) or
                (domain.toLong() shl DOMAIN_SHIFT)
        }

        fun requireKnownAnswers() {
            require(encode(1, 5, 1, 1) == KAT_ANDROID_ADAPTER)
            require(encode(2, 5, 1, 1) == KAT_WRONG_THREAD)
            require(encode(7, 5, 1, 1) == KAT_INVALID_HANDLE)
            require(encode(8, 5, 1, 1) == KAT_STALE_HANDLE)
            require(encode(9, 5, 1, 1) == KAT_CROSS_RUNTIME)
            require(encode(11, 5, 1, 1) == KAT_REENTRANT)
            require(encode(7, 3, 1, 3) == KAT_DECLARED_FIXTURE)
            require(encode(1, 5, 0, 2) == KAT_UNEXPECTED_ADAPTER)
            for (kat in listOf(KAT_ANDROID_ADAPTER, KAT_DECLARED_FIXTURE, KAT_UNEXPECTED_ADAPTER)) {
                val decoded = decode(kat)
                require(encode(decoded.code, decoded.statusClass, decoded.retry, decoded.domain) == kat)
            }
        }

        fun decode(raw: Long): StatusWord = StatusWord(
            raw = raw,
            code = raw and 0xffff_ffffL,
            statusClass = ((raw ushr CLASS_SHIFT) and 0x7L).toInt(),
            retry = ((raw ushr RETRY_SHIFT) and 0x3L).toInt(),
            domain = ((raw ushr DOMAIN_SHIFT) and 0xffffL).toInt(),
        )
    }

    fun isPrecommitAndroidRejection(): Boolean =
        domain == DOMAIN_ANDROID && (code and 0x8000_0000L) == 0L
}
