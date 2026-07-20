package io.github.lamggm.auxscreen

internal object ReconnectPolicy {
    private val delaysSeconds = intArrayOf(1, 2, 4, 8, 15)

    fun delayForAttempt(attempt: Int): Int? = delaysSeconds.getOrNull(attempt)

    fun isRetryableHostError(code: String): Boolean = code == "busy" || code == "heartbeat_timeout"
}

internal class TrackIdRegistry {
    private val ids = mutableSetOf<String>()

    fun markIfNew(id: String): Boolean = ids.add(id)

    fun clear() = ids.clear()
}
