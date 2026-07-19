package io.github.lamggm.auxscreen

import android.os.Bundle
import android.os.Build
import android.view.View
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.viewmodel.compose.viewModel
import org.webrtc.RendererCommon
import org.webrtc.SurfaceViewRenderer

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        enableEdgeToEdge()
        enterImmersiveMode()
        val acceptsTestExtras = BuildConfig.DEBUG || BuildConfig.BUILD_TYPE == "personal"
        val debugEndpoint = intent.getStringExtra("endpoint").takeIf { acceptsTestExtras }
        val debugToken = intent.getStringExtra("token").takeIf { acceptsTestExtras }
        setContent { AuxScreenApp(debugEndpoint, debugToken) }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) enterImmersiveMode()
    }

    @Suppress("DEPRECATION")
    private fun enterImmersiveMode() {
        window.decorView.systemUiVisibility =
            View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or
                View.SYSTEM_UI_FLAG_FULLSCREEN or
                View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
                View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
                View.SYSTEM_UI_FLAG_LAYOUT_STABLE
    }
}

@Composable
private fun AuxScreenApp(debugEndpoint: String?, debugToken: String?) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Color(0xFF71D2FF),
            background = Color(0xFF070A0F),
            surface = Color(0xFF10151D),
        ),
    ) {
        val context = LocalContext.current
        val controller: AuxScreenController = viewModel { AuxScreenController(context) }
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            when (val state = controller.state) {
                is ConnectionState.Streaming -> StreamingScreen(controller, state)
                else -> ConnectionScreen(controller, state, debugEndpoint, debugToken)
            }
        }
    }
}

@Composable
private fun ConnectionScreen(
    controller: AuxScreenController,
    state: ConnectionState,
    debugEndpoint: String?,
    debugToken: String?,
) {
    val context = LocalContext.current
    val preferences = remember(context) {
        context.getSharedPreferences("auxscreen", android.content.Context.MODE_PRIVATE)
    }
    var endpoint by rememberSaveable {
        mutableStateOf(
            debugEndpoint
                ?: preferences.getString("endpoint", null)
                ?: "ws://192.168.1.254:9898/v1/session",
        )
    }
    var token by rememberSaveable { mutableStateOf(debugToken.orEmpty()) }
    val metrics = LocalResources.current.displayMetrics

    LaunchedEffect(debugEndpoint, debugToken) {
        if (!debugEndpoint.isNullOrBlank() && !debugToken.isNullOrBlank()) {
            controller.connect(debugEndpoint, debugToken, metrics.widthPixels, metrics.heightPixels)
        }
    }

    Box(modifier = Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
        Column(
            modifier = Modifier.fillMaxWidth(0.66f),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("AuxScreen", style = MaterialTheme.typography.displaySmall, fontWeight = FontWeight.Black)
            Text("${Build.MANUFACTURER} ${Build.MODEL} · ${BuildConfig.VERSION_NAME}", color = Color(0xFFA9B4C4))
            Spacer(Modifier.height(28.dp))
            OutlinedTextField(
                value = endpoint,
                onValueChange = { endpoint = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("WebSocket do host") },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                enabled = state !is ConnectionState.Connecting && state !is ConnectionState.Reconnecting,
            )
            Spacer(Modifier.height(12.dp))
            OutlinedTextField(
                value = token,
                onValueChange = { token = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("Token da sessão") },
                enabled = state !is ConnectionState.Connecting && state !is ConnectionState.Reconnecting,
            )
            Spacer(Modifier.height(20.dp))
            Button(
                onClick = {
                    preferences.edit().putString("endpoint", endpoint.trim()).apply()
                    controller.connect(
                        endpoint = endpoint,
                        token = token,
                        width = metrics.widthPixels,
                        height = metrics.heightPixels,
                    )
                },
                enabled = state !is ConnectionState.Connecting && state !is ConnectionState.Reconnecting,
                colors = ButtonDefaults.buttonColors(contentColor = Color(0xFF071018)),
            ) {
                Text(if (state is ConnectionState.Connecting) "Conectando…" else "Usar como monitor")
            }
            Spacer(Modifier.height(16.dp))
            when (state) {
                is ConnectionState.Error -> Text(state.message, color = MaterialTheme.colorScheme.error)
                is ConnectionState.Connecting -> Text("Negociando WebRTC e H.264…", color = Color(0xFFA9B4C4))
                is ConnectionState.Reconnecting -> Text(
                    "Reconectando ${state.attempt}/5 em ${state.delaySeconds}s · ${state.reason}",
                    color = Color(0xFFFFC56E),
                )
                else -> Text("Sem câmera, microfone ou coleta de dados.", color = Color(0xFF7E8998))
            }
        }
    }
}

@Composable
private fun StreamingScreen(controller: AuxScreenController, state: ConnectionState.Streaming) {
    var controlsVisible by remember { mutableStateOf(true) }
    LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(3500)
        controlsVisible = false
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
            .pointerInput(Unit) {
                detectTapGestures { controlsVisible = !controlsVisible }
            },
    ) {
        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { context ->
                SurfaceViewRenderer(context).also { view ->
                    view.setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                    controller.attachRenderer(view)
                }
            },
            onRelease = controller::detachRenderer,
        )

        if (controlsVisible) {
            Row(
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(20.dp)
                    .background(Color(0xCC10151D), RoundedCornerShape(12.dp))
                    .padding(horizontal = 14.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val stats = state.stats
                Text(
                    buildString {
                        append(state.details)
                        if (stats.renderedFps > 0) append(" · %.1f render".format(stats.renderedFps))
                        stats.rttMs?.let { append(" · %.0f ms RTT".format(it)) }
                        val received = stats.packetsReceived
                        val lost = stats.packetsLost
                        if (received != null && lost != null && received + lost > 0) {
                            val loss = lost.coerceAtLeast(0) * 100.0 / (received + lost)
                            append(" · %.2f%% rede".format(loss))
                        }
                        if (stats.framesDropped > 0) append(" · ${stats.framesDropped} drops")
                    },
                    color = Color.White,
                )
                Spacer(Modifier.width(12.dp))
                Button(onClick = controller::disconnect) { Text("Desconectar") }
            }
        }
    }
}
