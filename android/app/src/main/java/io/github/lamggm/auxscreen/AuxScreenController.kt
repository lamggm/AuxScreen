package io.github.lamggm.auxscreen

import android.annotation.SuppressLint
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.webrtc.AudioTrack
import org.webrtc.CandidatePairChangeEvent
import org.webrtc.DataChannel
import org.webrtc.EglBase
import org.webrtc.IceCandidate
import org.webrtc.MediaConstraints
import org.webrtc.MediaStream
import org.webrtc.MediaStreamTrack
import org.webrtc.PeerConnection
import org.webrtc.PeerConnectionFactory
import org.webrtc.RendererCommon
import org.webrtc.RTCStatsReport
import org.webrtc.RtpReceiver
import org.webrtc.RtpTransceiver
import org.webrtc.SdpObserver
import org.webrtc.SessionDescription
import org.webrtc.SurfaceViewRenderer
import org.webrtc.VideoTrack
import java.util.concurrent.TimeUnit

internal data class StreamStats(
    val renderedFps: Double = 0.0,
    val framesDecoded: Long? = null,
    val framesDropped: Long = 0,
    val bitrateKbps: Double? = null,
    val jitterMs: Double? = null,
    val rttMs: Double? = null,
    val packetsReceived: Long? = null,
    val packetsLost: Long? = null,
    val jitterBufferDelayMs: Double? = null,
    val jitterBufferTargetDelayMs: Double? = null,
    val jitterBufferMinimumDelayMs: Double? = null,
    val decodeTimeMs: Double? = null,
    val processingDelayMs: Double? = null,
    val decoder: String? = null,
)

internal sealed interface ConnectionState {
    data object Idle : ConnectionState
    data object Connecting : ConnectionState
    data class Reconnecting(val attempt: Int, val delaySeconds: Int, val reason: String) : ConnectionState
    data class Streaming(val details: String, val stats: StreamStats = StreamStats()) : ConnectionState
    data class Error(val message: String) : ConnectionState
}

internal class AuxScreenController(context: Context) : ViewModel() {
    private data class ConnectionParams(
        val endpoint: String,
        val token: String,
        val width: Int,
        val height: Int,
    )

    private val appContext = context.applicationContext
    private val mainHandler = Handler(Looper.getMainLooper())
    private val eglBase = EglBase.create()
    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(5, TimeUnit.SECONDS)
        .pingInterval(10, TimeUnit.SECONDS)
        .build()
    private val peerFactory: PeerConnectionFactory

    private var peerConnection: PeerConnection? = null
    private var webSocket: WebSocket? = null
    // AndroidView calls detachRenderer before its Activity-owned view is destroyed.
    @SuppressLint("StaticFieldLeak")
    private var renderer: SurfaceViewRenderer? = null
    private var remoteVideo: VideoTrack? = null
    private val remoteTrackIds = TrackIdRegistry()
    private var streamConfig: HostMessage.StreamConfig? = null
    private var connectionParams: ConnectionParams? = null
    private var generation = 0L
    private var reconnectAttempts = 0
    private var manualDisconnect = false
    private var closed = false
    private var lastFramesDecoded: Long? = null
    private var lastBytesReceived: Long? = null
    private var lastStatsTimestampUs: Double? = null

    var state: ConnectionState by mutableStateOf(ConnectionState.Idle)
        private set

    init {
        val initialization = PeerConnectionFactory.InitializationOptions.builder(appContext)
            .setEnableInternalTracer(false)
        if (BuildConfig.FORCE_ZERO_PLAYOUT_DELAY) {
            initialization.setFieldTrials(
                "WebRTC-ForcePlayoutDelay/min_ms:0,max_ms:0/" +
                    "WebRTC-ZeroPlayoutDelay/min_pacing:8ms,max_decode_queue_size:1/",
            )
            Log.w(TAG, "zero playout delay forced for personal/debug low-latency build")
        }
        PeerConnectionFactory.initialize(initialization.createInitializationOptions())
        peerFactory = PeerConnectionFactory.builder()
            .setVideoDecoderFactory(org.webrtc.DefaultVideoDecoderFactory(eglBase.eglBaseContext))
            .setVideoEncoderFactory(org.webrtc.DefaultVideoEncoderFactory(eglBase.eglBaseContext, true, false))
            .createPeerConnectionFactory()
    }

    fun attachRenderer(view: SurfaceViewRenderer) {
        if (renderer === view) return
        renderer?.let { old -> remoteVideo?.removeSink(old) }
        renderer = view
        view.init(eglBase.eglBaseContext, null)
        view.setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
        // SurfaceView's fixed-size hardware scaler can leave a 16:9 surface
        // larger than the 16:10 AndroidView and crop both horizontal edges.
        // Let the EGL renderer letterbox inside the view instead.
        view.setEnableHardwareScaler(false)
        view.setMirror(false)
        remoteVideo?.addSink(view)
    }

    fun detachRenderer(view: SurfaceViewRenderer) {
        if (renderer !== view) return
        remoteVideo?.removeSink(view)
        renderer = null
        view.release()
    }

    fun connect(endpoint: String, token: String, width: Int, height: Int) {
        val normalized = EndpointPolicy.validate(endpoint, BuildConfig.ALLOW_LAN_CLEARTEXT)
            .getOrElse {
                state = ConnectionState.Error(it.message ?: "Endereço inválido")
                return
            }
        connectionParams = ConnectionParams(normalized, token.trim(), width, height)
        manualDisconnect = false
        reconnectAttempts = 0
        state = ConnectionState.Connecting
        connectTransport()
    }

    private fun connectTransport() {
        val params = connectionParams ?: return
        generation += 1
        val callbackGeneration = generation
        disconnectTransport()
        val request = Request.Builder().url(params.endpoint).build()
        webSocket = httpClient.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) = onMain(callbackGeneration) {
                webSocket.send(
                    SignalingProtocol.clientHello(
                        token = params.token,
                        model = "${Build.MANUFACTURER} ${Build.MODEL}",
                        width = params.width,
                        height = params.height,
                        maxFps = 60,
                    ),
                )
            }

            override fun onMessage(webSocket: WebSocket, text: String) = onMain(callbackGeneration) {
                runCatching { handleHostMessage(SignalingProtocol.parse(text)) }
                    .onFailure { fail("Sinalização inválida: ${it.message}", retryable = false) }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) =
                onMain(callbackGeneration) {
                    scheduleReconnect("WebSocket falhou: ${t.message ?: t.javaClass.simpleName}")
                }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) =
                onMain(callbackGeneration) {
                    if (!manualDisconnect) {
                        scheduleReconnect("Host encerrou a sessão ($code${reason.takeIf(String::isNotBlank)?.let { ": $it" } ?: ""})")
                    }
                }
        })
    }

    private fun onMain(expectedGeneration: Long, block: () -> Unit) {
        mainHandler.post {
            if (!closed && generation == expectedGeneration) block()
        }
    }

    private fun handleHostMessage(message: HostMessage) {
        when (message) {
            is HostMessage.HelloAck -> if (message.protocol != PROTOCOL_VERSION) {
                fail("Versão de protocolo incompatível: ${message.protocol}", retryable = false)
            }
            is HostMessage.StreamConfig -> {
                streamConfig = message
                Log.i(TAG, "stream ${message.encodedWidth}x${message.encodedHeight}@${message.fps}, ${message.bitrateKbps} kbps")
            }
            is HostMessage.Offer -> acceptOffer(message.sdp)
            is HostMessage.IceCandidate -> peerConnection?.addIceCandidate(
                IceCandidate(message.sdpMid, message.sdpMLineIndex, message.candidate),
            )
            is HostMessage.Ping -> webSocket?.send(SignalingProtocol.pong(message.nonce))
            is HostMessage.Pong -> Unit
            is HostMessage.Error -> fail(
                "${message.code}: ${message.message}",
                retryable = ReconnectPolicy.isRetryableHostError(message.code),
            )
        }
    }

    private fun acceptOffer(sdp: String) {
        if (peerConnection == null) peerConnection = createPeerConnection()
        val peer = peerConnection ?: return fail("Não foi possível criar o PeerConnection", false)
        peer.setRemoteDescription(object : SimpleSdpObserver() {
            override fun onSetSuccess() {
                peer.createAnswer(object : SimpleSdpObserver() {
                    override fun onCreateSuccess(description: SessionDescription) {
                        peer.setLocalDescription(object : SimpleSdpObserver() {
                            override fun onSetSuccess() {
                                webSocket?.send(SignalingProtocol.answer(description.description))
                            }
                            override fun onSetFailure(error: String) = fail("Falha ao aplicar answer: $error", false)
                        }, description)
                    }
                    override fun onCreateFailure(error: String) = fail("Falha ao criar answer: $error", false)
                }, MediaConstraints())
            }
            override fun onSetFailure(error: String) = fail("Falha ao aplicar offer: $error", false)
        }, SessionDescription(SessionDescription.Type.OFFER, sdp))
    }

    private fun createPeerConnection(): PeerConnection? {
        val rtcConfig = PeerConnection.RTCConfiguration(emptyList()).apply {
            sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
            continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_CONTINUALLY
            bundlePolicy = PeerConnection.BundlePolicy.MAXBUNDLE
        }
        return peerFactory.createPeerConnection(rtcConfig, object : PeerConnection.Observer {
            override fun onSignalingChange(state: PeerConnection.SignalingState) = Unit
            override fun onIceConnectionChange(state: PeerConnection.IceConnectionState) {
                Log.i(TAG, "ICE state: $state")
                if (state == PeerConnection.IceConnectionState.FAILED ||
                    state == PeerConnection.IceConnectionState.DISCONNECTED
                ) {
                    mainHandler.post { if (!manualDisconnect) scheduleReconnect("Rede WebRTC interrompida") }
                }
            }
            override fun onStandardizedIceConnectionChange(state: PeerConnection.IceConnectionState) = Unit
            override fun onConnectionChange(newState: PeerConnection.PeerConnectionState) = Unit
            override fun onIceConnectionReceivingChange(receiving: Boolean) = Unit
            override fun onIceGatheringChange(state: PeerConnection.IceGatheringState) = Unit
            override fun onIceCandidate(candidate: IceCandidate) {
                webSocket?.send(SignalingProtocol.iceCandidate(candidate.sdp, candidate.sdpMid, candidate.sdpMLineIndex))
            }
            override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>) = Unit
            override fun onSelectedCandidatePairChanged(event: CandidatePairChangeEvent) = Unit
            override fun onAddStream(stream: MediaStream) = Unit
            override fun onRemoveStream(stream: MediaStream) = Unit
            override fun onDataChannel(channel: DataChannel) = Unit
            override fun onRenegotiationNeeded() = Unit
            override fun onAddTrack(receiver: RtpReceiver, mediaStreams: Array<out MediaStream>) {
                mainHandler.post { attachRemoteTrack(receiver.track()) }
            }
            override fun onTrack(transceiver: RtpTransceiver) {
                mainHandler.post { attachRemoteTrack(transceiver.receiver.track()) }
            }
        })
    }

    private fun attachRemoteTrack(track: MediaStreamTrack?) {
        if (track is AudioTrack) track.setEnabled(false)
        if (track !is VideoTrack || !remoteTrackIds.markIfNew(track.id())) return
        remoteVideo?.let { previous -> renderer?.let(previous::removeSink) }
        remoteVideo = track
        renderer?.let(track::addSink)
        reconnectAttempts = 0
        val config = streamConfig
        state = ConnectionState.Streaming(
            if (config == null) "H.264 recebido" else "${config.encodedWidth}×${config.encodedHeight} · ${config.fps} FPS",
        )
        startStats()
        Log.i(TAG, "remote video track registered: ${track.id()}")
    }

    private fun startStats() {
        mainHandler.removeCallbacks(statsRunnable)
        mainHandler.post(statsRunnable)
    }

    private val statsRunnable = object : Runnable {
        override fun run() {
            val peer = peerConnection
            if (peer != null && state is ConnectionState.Streaming) {
                peer.getStats { report -> mainHandler.post { consumeStats(report) } }
                mainHandler.postDelayed(this, 1_000)
            }
        }
    }

    private fun consumeStats(report: RTCStatsReport) {
        val inbound = report.statsMap.values.firstOrNull { stat ->
            stat.type == "inbound-rtp" &&
                (stat.members["kind"] == "video" || stat.members["mediaType"] == "video")
        } ?: return
        val decoded = (inbound.members["framesDecoded"] as? Number)?.toLong()
        val dropped = (inbound.members["framesDropped"] as? Number)?.toLong() ?: 0L
        val bytes = (inbound.members["bytesReceived"] as? Number)?.toLong()
        val packetsReceived = (inbound.members["packetsReceived"] as? Number)?.toLong()
        val packetsLost = (inbound.members["packetsLost"] as? Number)?.toLong()
        val previousDecoded = lastFramesDecoded
        val previousBytes = lastBytesReceived
        val previousTimestamp = lastStatsTimestampUs
        val elapsedSeconds = previousTimestamp?.let { (report.timestampUs - it) / 1_000_000.0 }
        val fps = if (decoded != null && previousDecoded != null && elapsedSeconds != null && elapsedSeconds > 0) {
            (decoded - previousDecoded) / elapsedSeconds
        } else 0.0
        val bitrate = if (bytes != null && previousBytes != null && elapsedSeconds != null && elapsedSeconds > 0) {
            (bytes - previousBytes) * 8.0 / elapsedSeconds / 1_000.0
        } else null
        val jitterMs = (inbound.members["jitter"] as? Number)?.toDouble()?.times(1_000.0)
        val emitted = (inbound.members["jitterBufferEmittedCount"] as? Number)?.toDouble()
        fun averagePerFrame(member: String, count: Double?): Double? {
            val totalSeconds = (inbound.members[member] as? Number)?.toDouble() ?: return null
            if (count == null || count <= 0.0) return null
            return totalSeconds * 1_000.0 / count
        }
        val jitterBufferDelayMs = averagePerFrame("jitterBufferDelay", emitted)
        val jitterBufferTargetDelayMs = averagePerFrame("jitterBufferTargetDelay", emitted)
        val jitterBufferMinimumDelayMs = averagePerFrame("jitterBufferMinimumDelay", emitted)
        val decodedCount = decoded?.toDouble()
        val decodeTimeMs = averagePerFrame("totalDecodeTime", decodedCount)
        val processingDelayMs = averagePerFrame("totalProcessingDelay", decodedCount)
        val decoder = inbound.members["decoderImplementation"] as? String
        val rttMs = report.statsMap.values
            .firstOrNull { it.type == "candidate-pair" && it.members["state"] == "succeeded" }
            ?.members?.get("currentRoundTripTime")
            .let { (it as? Number)?.toDouble()?.times(1_000.0) }
        lastFramesDecoded = decoded
        lastBytesReceived = bytes
        lastStatsTimestampUs = report.timestampUs
        val stats = StreamStats(
            fps,
            decoded,
            dropped,
            bitrate,
            jitterMs,
            rttMs,
            packetsReceived,
            packetsLost,
            jitterBufferDelayMs,
            jitterBufferTargetDelayMs,
            jitterBufferMinimumDelayMs,
            decodeTimeMs,
            processingDelayMs,
            decoder,
        )
        val current = state as? ConnectionState.Streaming ?: return
        state = current.copy(stats = stats)
        webSocket?.send(
            SignalingProtocol.clientStats(
                fps,
                decoded,
                dropped,
                bitrate,
                jitterMs,
                rttMs,
                packetsReceived,
                packetsLost,
                jitterBufferDelayMs,
                jitterBufferTargetDelayMs,
                jitterBufferMinimumDelayMs,
                decodeTimeMs,
                processingDelayMs,
                decoder,
            ),
        )
    }

    private fun scheduleReconnect(reason: String) {
        if (manualDisconnect || closed) return
        generation += 1
        disconnectTransport()
        val delay = ReconnectPolicy.delayForAttempt(reconnectAttempts)
        if (delay == null) {
            state = ConnectionState.Error("Não foi possível reconectar: $reason")
            return
        }
        reconnectAttempts += 1
        val scheduledGeneration = generation
        state = ConnectionState.Reconnecting(reconnectAttempts, delay, reason)
        mainHandler.postDelayed({
            if (!closed && !manualDisconnect && generation == scheduledGeneration) {
                state = ConnectionState.Connecting
                connectTransport()
            }
        }, delay * 1_000L)
    }

    private fun fail(message: String, retryable: Boolean) {
        Log.e(TAG, message)
        if (retryable) scheduleReconnect(message) else {
            generation += 1
            disconnectTransport()
            state = ConnectionState.Error(message)
        }
    }

    fun disconnect() {
        manualDisconnect = true
        generation += 1
        disconnectTransport()
        state = ConnectionState.Idle
    }

    private fun disconnectTransport() {
        mainHandler.removeCallbacks(statsRunnable)
        remoteVideo?.let { video -> renderer?.let(video::removeSink) }
        remoteVideo = null
        peerConnection?.close()
        peerConnection?.dispose()
        peerConnection = null
        remoteTrackIds.clear()
        webSocket?.close(1000, "client disconnect")
        webSocket = null
        streamConfig = null
        lastFramesDecoded = null
        lastBytesReceived = null
        lastStatsTimestampUs = null
    }

    override fun onCleared() {
        closed = true
        disconnect()
        renderer?.release()
        renderer = null
        peerFactory.dispose()
        eglBase.release()
        httpClient.dispatcher.executorService.shutdown()
        super.onCleared()
    }

    private open inner class SimpleSdpObserver : SdpObserver {
        override fun onCreateSuccess(description: SessionDescription) = Unit
        override fun onSetSuccess() = Unit
        override fun onCreateFailure(error: String) {
            mainHandler.post { fail(error, false) }
        }
        override fun onSetFailure(error: String) {
            mainHandler.post { fail(error, false) }
        }
    }

    private companion object {
        const val TAG = "AuxScreen"
    }
}
