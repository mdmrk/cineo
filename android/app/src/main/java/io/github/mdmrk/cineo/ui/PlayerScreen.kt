package io.github.mdmrk.cineo.ui

import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import io.github.mdmrk.cineo.Native
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.PlaybackView
import io.github.mdmrk.cineo.rust.PlayerStatusView
import io.github.mdmrk.cineo.rust.TorrentView
import kotlinx.coroutines.delay

@Composable
fun PlayerScreen(playback: PlaybackView, torrent: TorrentView?, cineo: Cineo) {
    var status by remember { mutableStateOf<PlayerStatusView?>(null) }
    var controls by remember { mutableStateOf(true) }
    var dragging by remember { mutableStateOf<Float?>(null) }
    LaunchedEffect(Unit) {
        while (true) {
            status = cineo.playerStatus()
            delay(250)
        }
    }
    LaunchedEffect(controls, status?.paused) {
        if (controls && status?.paused == false) {
            delay(3000)
            controls = false
        }
    }
    BackHandler { cineo.stopPlayback() }
    FullScreen()
    Box(
        Modifier
            .fillMaxSize()
            .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {
                controls = !controls
            },
    ) {
        AndroidView(
            factory = { context ->
                SurfaceView(context).apply {
                    keepScreenOn = true
                    holder.addCallback(SurfaceCallback(cineo))
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
        val current = status
        if (current == null || !current.loaded || current.waiting) {
            CircularProgressIndicator(Modifier.align(Alignment.Center))
        }
        if (controls && current != null) {
            Controls(
                title = playback.title,
                torrent = torrent,
                status = current,
                position = dragging ?: current.positionS.toFloat(),
                onSeekDrag = { dragging = it },
                onSeekDone = {
                    dragging?.let { cineo.seekTo(it.toDouble()) }
                    dragging = null
                },
                onBack = { cineo.stopPlayback() },
                onTogglePause = { cineo.togglePause() },
                seekStep = playback.seekStepS.toInt(),
                onSeekBy = { cineo.seekBy(it.toDouble()) },
            )
        }
    }
}

@Composable
private fun Controls(
    title: String,
    torrent: TorrentView?,
    status: PlayerStatusView,
    position: Float,
    onSeekDrag: (Float) -> Unit,
    onSeekDone: () -> Unit,
    onBack: () -> Unit,
    onTogglePause: () -> Unit,
    seekStep: Int,
    onSeekBy: (Int) -> Unit,
) {
    val white = Color.White
    Column(
        Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.4f)).safeDrawingPadding().padding(16.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onBack) { Text(stringResource(R.string.back), color = white) }
            Text(title, color = white)
        }
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            TextButton(onClick = { onSeekBy(-seekStep) }) {
                Text("−$seekStep", color = white)
            }
            TextButton(onClick = onTogglePause) {
                Text(stringResource(if (status.paused) R.string.play else R.string.pause), color = white)
            }
            TextButton(onClick = { onSeekBy(seekStep) }) {
                Text("+$seekStep", color = white)
            }
        }
        Column {
            TorrentStats(torrent)
            val duration = status.durationS.toFloat().coerceAtLeast(0f)
            Slider(
                value = position.coerceIn(0f, duration.coerceAtLeast(position)),
                onValueChange = onSeekDrag,
                onValueChangeFinished = onSeekDone,
                valueRange = 0f..duration.coerceAtLeast(1f),
                enabled = duration > 0f,
            )
            Text("${clock(position.toDouble())} / ${clock(status.durationS)}", color = white)
        }
    }
}

@Composable
private fun FullScreen() {
    val view = LocalView.current
    DisposableEffect(view) {
        val window = (view.context as? android.app.Activity)?.window
        val insets = window?.let { WindowCompat.getInsetsController(it, view) }
        insets?.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        insets?.hide(WindowInsetsCompat.Type.systemBars())
        onDispose { insets?.show(WindowInsetsCompat.Type.systemBars()) }
    }
}

private class SurfaceCallback(private val cineo: Cineo) : SurfaceHolder.Callback {
    override fun surfaceCreated(holder: SurfaceHolder) {
        cineo.attachSurface(Native.attachSurface(holder.surface))
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        cineo.resizeSurface(width.toUInt(), height.toUInt())
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        cineo.detachSurface()
        Native.releaseSurface()
    }
}

private fun clock(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val (h, m, s) = Triple(total / 3600, total / 60 % 60, total % 60)
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}
