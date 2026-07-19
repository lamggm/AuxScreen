package io.github.lamggm.auxscreen

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class EndpointPolicyTest {
    @Test
    fun acceptsPrivateLanWebSocket() {
        assertEquals(
            "ws://192.168.1.254:9898/v1/session",
            EndpointPolicy.validate("ws://192.168.1.254:9898/v1/session/", true).getOrThrow(),
        )
    }

    @Test
    fun rejectsPublicCleartextAddress() {
        assertTrue(EndpointPolicy.validate("ws://8.8.8.8:9898/v1/session", true).isFailure)
    }

    @Test
    fun releasePolicyRequiresWss() {
        assertTrue(EndpointPolicy.validate("ws://192.168.1.254:9898/v1/session", false).isFailure)
        assertTrue(EndpointPolicy.validate("wss://auxscreen.example/v1/session", false).isSuccess)
    }
}
