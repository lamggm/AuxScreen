package io.github.lamggm.auxscreen

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ConnectionPoliciesTest {
    @Test
    fun reconnectUsesBoundedBackoff() {
        assertEquals(listOf(1, 2, 4, 8, 15), (0..4).map(ReconnectPolicy::delayForAttempt))
        assertNull(ReconnectPolicy.delayForAttempt(5))
        assertTrue(ReconnectPolicy.isRetryableHostError("busy"))
        assertTrue(ReconnectPolicy.isRetryableHostError("heartbeat_timeout"))
        assertFalse(ReconnectPolicy.isRetryableHostError("unauthorized"))
    }

    @Test
    fun trackIdsAreDeduplicatedWithinOneTransport() {
        val registry = TrackIdRegistry()
        assertTrue(registry.markIfNew("video-1"))
        assertFalse(registry.markIfNew("video-1"))
        assertTrue(registry.markIfNew("video-2"))
        registry.clear()
        assertTrue(registry.markIfNew("video-1"))
    }
}
