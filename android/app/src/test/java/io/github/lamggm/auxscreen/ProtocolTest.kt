package io.github.lamggm.auxscreen

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ProtocolTest {
    @Test
    fun clientHelloMatchesHostSchema() {
        val hello = SignalingProtocol.clientHello("abc", "SM-X400", 2112, 1320, 60)
        assertTrue(hello.contains("\"type\":\"client_hello\""))
        assertTrue(hello.contains("\"protocol\":1"))
        assertTrue(hello.contains("\"max_width\":2112"))
    }

    @Test
    fun clientHelloAllowsEmptyTokenForNoAuthHost() {
        val hello = org.json.JSONObject(
            SignalingProtocol.clientHello("", "SM-X400", 2112, 1320, 60),
        )
        assertEquals("", hello.getString("token"))
    }

    @Test
    fun parsesStreamConfig() {
        val parsed = SignalingProtocol.parse(
            """{"type":"stream_config","source_width":1920,"source_height":1200,"encoded_width":1920,"encoded_height":1200,"fps":30,"bitrate_kbps":6000}""",
        )
        assertEquals(
            HostMessage.StreamConfig(1920, 1200, 1920, 1200, 30, 6000),
            parsed,
        )
    }

    @Test(expected = IllegalStateException::class)
    fun rejectsUnknownMessage() {
        SignalingProtocol.parse("""{"type":"telepathy"}""")
    }

    @Test
    fun serializesExtendedClientStats() {
        val payload = SignalingProtocol.clientStats(
            29.5,
            100,
            2,
            5900.0,
            3.2,
            8.4,
            2841,
            0,
            18.4,
            22.0,
            10.0,
            2.1,
            25.0,
            null,
        )
        val json = org.json.JSONObject(payload)
        assertEquals("client_stats", json.getString("type"))
        assertEquals(100L, json.getLong("frames_decoded"))
        assertEquals(8.4, json.getDouble("rtt_ms"), 0.01)
        assertEquals(2841L, json.getLong("packets_received"))
        assertEquals(0L, json.getLong("packets_lost"))
        assertEquals(22.0, json.getDouble("jitter_buffer_target_delay_ms"), 0.01)
    }

    @Test
    fun sharedFixturesArePackagedForBothImplementations() {
        val fixture = javaClass.classLoader!!.getResource("client_hello.json")
        assertTrue("shared protocol fixture missing", fixture != null)
    }
}
