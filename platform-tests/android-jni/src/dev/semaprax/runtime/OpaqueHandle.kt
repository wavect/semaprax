package dev.semaprax.runtime

internal data class OpaqueHandle(val runtimeTag: Int, val generation: Int, val slot: Int) {
    init {
        require(runtimeTag in 1..MAX_TAG)
        require(generation in 1..MAX_COMPONENT)
        require(slot in 1..MAX_COMPONENT)
    }

    fun encode(): Long =
        (runtimeTag.toLong() shl TAG_SHIFT) or
            (generation.toLong() shl GENERATION_SHIFT) or
            slot.toLong()

    companion object {
        const val KNOWN_ANSWER = 0x0001000001000001L
        private const val TAG_SHIFT = 48
        private const val GENERATION_SHIFT = 24
        private const val MAX_TAG = 0x7fff
        private const val MAX_COMPONENT = 0x00ff_ffff

        fun decode(raw: Long): OpaqueHandle {
            require(raw > 0L) { "SPXAJH01 is zero or negative" }
            require(raw ushr 63 == 0L) { "SPXAJH01 reserved sign bit is set" }
            return OpaqueHandle(
                runtimeTag = ((raw ushr TAG_SHIFT) and MAX_TAG.toLong()).toInt(),
                generation = ((raw ushr GENERATION_SHIFT) and MAX_COMPONENT.toLong()).toInt(),
                slot = (raw and MAX_COMPONENT.toLong()).toInt(),
            )
        }

        fun requireKnownAnswer() {
            val fields = OpaqueHandle(1, 1, 1)
            require(fields.encode() == KNOWN_ANSWER)
            require(decode(KNOWN_ANSWER) == fields)
        }
    }
}
