package io.github.lamggm.auxscreen

import org.json.JSONObject

internal const val PROTOCOL_VERSION = 1

internal sealed interface HostMessage {
    data class HelloAck(val protocol: Int) : HostMessage
    data class Offer(val sdp: String) : HostMessage
    data class IceCandidate(
        val candidate: String,
        val sdpMid: String?,
        val sdpMLineIndex: Int,
    ) : HostMessage
    data class StreamConfig(
        val sourceWidth: Int,
        val sourceHeight: Int,
        val encodedWidth: Int,
        val encodedHeight: Int,
        val fps: Int,
        val bitrateKbps: Int,
    ) : HostMessage
    data class Ping(val nonce: Long) : HostMessage
    data class Pong(val nonce: Long) : HostMessage
    data class Error(val code: String, val message: String) : HostMessage
}

internal object SignalingProtocol {
    fun clientHello(token: String, model: String, width: Int, height: Int, maxFps: Int): String =
        JSONObject()
            .put("type", "client_hello")
            .put("protocol", PROTOCOL_VERSION)
            .put("token", token)
            .put("device", model)
            .put("max_width", width)
            .put("max_height", height)
            .put("max_fps", maxFps)
            .toString()

    fun answer(sdp: String): String = JSONObject()
        .put("type", "answer")
        .put("sdp", sdp)
        .toString()

    fun iceCandidate(candidate: String, sdpMid: String?, sdpMLineIndex: Int): String =
        JSONObject()
            .put("type", "ice_candidate")
            .put("candidate", candidate)
            .put("sdp_mid", sdpMid ?: JSONObject.NULL)
            .put("sdp_mline_index", sdpMLineIndex)
            .toString()

    fun pong(nonce: Long): String = JSONObject()
        .put("type", "pong")
        .put("nonce", nonce)
        .toString()

    fun clientStats(
        renderedFps: Double,
        framesDecoded: Long?,
        framesDropped: Long,
        bitrateKbps: Double?,
        jitterMs: Double?,
        rttMs: Double?,
        packetsReceived: Long?,
        packetsLost: Long?,
        jitterBufferDelayMs: Double?,
        jitterBufferTargetDelayMs: Double?,
        jitterBufferMinimumDelayMs: Double?,
        decodeTimeMs: Double?,
        processingDelayMs: Double?,
        decoder: String?,
    ): String = JSONObject()
        .put("type", "client_stats")
        .put("rendered_fps", renderedFps)
        .put("frames_decoded", framesDecoded ?: JSONObject.NULL)
        .put("frames_dropped", framesDropped)
        .put("bitrate_kbps", bitrateKbps ?: JSONObject.NULL)
        .put("jitter_ms", jitterMs ?: JSONObject.NULL)
        .put("rtt_ms", rttMs ?: JSONObject.NULL)
        .put("packets_received", packetsReceived ?: JSONObject.NULL)
        .put("packets_lost", packetsLost ?: JSONObject.NULL)
        .put("jitter_buffer_delay_ms", jitterBufferDelayMs ?: JSONObject.NULL)
        .put("jitter_buffer_target_delay_ms", jitterBufferTargetDelayMs ?: JSONObject.NULL)
        .put("jitter_buffer_minimum_delay_ms", jitterBufferMinimumDelayMs ?: JSONObject.NULL)
        .put("decode_time_ms", decodeTimeMs ?: JSONObject.NULL)
        .put("processing_delay_ms", processingDelayMs ?: JSONObject.NULL)
        .put("decoder", decoder ?: JSONObject.NULL)
        .toString()

    fun parse(payload: String): HostMessage {
        val json = JSONObject(payload)
        return when (val type = json.getString("type")) {
            "hello_ack" -> HostMessage.HelloAck(json.getInt("protocol"))
            "offer" -> HostMessage.Offer(json.getString("sdp"))
            "ice_candidate" -> HostMessage.IceCandidate(
                candidate = json.getString("candidate"),
                sdpMid = json.optString("sdp_mid").takeIf { it.isNotBlank() && it != "null" },
                sdpMLineIndex = json.getInt("sdp_mline_index"),
            )
            "stream_config" -> HostMessage.StreamConfig(
                sourceWidth = json.getInt("source_width"),
                sourceHeight = json.getInt("source_height"),
                encodedWidth = json.getInt("encoded_width"),
                encodedHeight = json.getInt("encoded_height"),
                fps = json.getInt("fps"),
                bitrateKbps = json.getInt("bitrate_kbps"),
            )
            "ping" -> HostMessage.Ping(json.getLong("nonce"))
            "pong" -> HostMessage.Pong(json.getLong("nonce"))
            "error" -> HostMessage.Error(json.getString("code"), json.getString("message"))
            else -> error("Unknown signaling message: $type")
        }
    }
}
