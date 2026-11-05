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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.sp
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
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle

@Composable
fun PlayerScreen(playback: PlaybackView, torrent: TorrentView?, images: Images, cineo: Cineo) {
    var status by remember { mutableStateOf<PlayerStatusView?>(null) }
    var controls by remember { mutableStateOf(true) }
    var dragging by remember { mutableStateOf<Float?>(null) }
    var stats by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            while (true) {
                status = withContext(Dispatchers.Default) { cineo.playerStatus() }
                delay(250)
            }
        }
    }
    LaunchedEffect(controls, status?.paused, menu) {
        if (controls && !menu && status?.paused == false) {
            delay(playback.hideControlsMs.toLong())
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
            LoadingBackdrop(playback.art, images, firstLoad = current == null || !current.loaded)
        }
        if (controls && current != null) {
            Controls(
                title = playback.art.title,
                status = current,
                position = dragging ?: current.positionS.toFloat(),
                onSeekDrag = { dragging = it },
                onSeekDone = {
                    dragging?.let { cineo.seekTo(it.toDouble()) }
                    dragging = null
                },
                onBack = { cineo.stopPlayback() },
                onTogglePause = {
                    cineo.togglePause()
                    status = current.copy(paused = !current.paused)
                },
                seekStep = playback.seekStepS.toInt(),
                onSeekBy = { cineo.seekBy(it.toDouble()) },
                onToggleStats = { stats = !stats },
                onAudio = { cineo.selectAudio(it) },
                onSubtitle = { cineo.selectSubtitle(it) },
                onAddonSubtitle = { cineo.selectAddonSubtitle(it) },
                onMenu = { menu = it },
            )
        }
        if (stats && current != null) {
            StatsPanel(
                torrent,
                current.stats,
                Modifier.align(Alignment.TopStart).safeDrawingPadding().padding(start = 16.dp, top = 72.dp),
            )
        }
    }
}

@Composable
private fun Controls(
    title: String,
    status: PlayerStatusView,
    position: Float,
    onSeekDrag: (Float) -> Unit,
    onSeekDone: () -> Unit,
    onBack: () -> Unit,
    onTogglePause: () -> Unit,
    seekStep: Int,
    onSeekBy: (Int) -> Unit,
    onToggleStats: () -> Unit,
    onAudio: (Long) -> Unit,
    onSubtitle: (Long?) -> Unit,
    onAddonSubtitle: (String) -> Unit,
    onMenu: (Boolean) -> Unit,
) {
    val (back, forward) = when (seekStep) {
        5 -> Icon.REWIND_BACKWARD_5 to Icon.REWIND_FORWARD_5
        15 -> Icon.REWIND_BACKWARD_15 to Icon.REWIND_FORWARD_15
        30 -> Icon.REWIND_BACKWARD_30 to Icon.REWIND_FORWARD_30
        else -> Icon.REWIND_BACKWARD_10 to Icon.REWIND_FORWARD_10
    }
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier.fillMaxSize().background(
                Brush.verticalGradient(
                    0f to Color.Black.copy(alpha = 0.7f),
                    0.25f to Color.Transparent,
                    0.7f to Color.Transparent,
                    1f to Color.Black.copy(alpha = 0.8f),
                ),
            ),
        )
        Row(
            Modifier.align(Alignment.TopStart).fillMaxWidth().safeDrawingPadding().padding(8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            IconButton(onClick = onBack) { CineoIcon(Icon.ARROW_LEFT, tint = Color.White, label = stringResource(R.string.back)) }
            Text(
                title,
                color = Color.White,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            SubtitlesMenu(status, onTrack = onSubtitle, onAddon = onAddonSubtitle, onOpen = onMenu)
            AudioMenu(status, onTrack = onAudio, onOpen = onMenu)
            val statsLabel = stringResource(R.string.player_stats)
            IconButton(onClick = onToggleStats, modifier = Modifier.semantics { contentDescription = statsLabel }) {
                CineoIcon(Icon.ACTIVITY, tint = Color.White)
            }
        }
        Row(
            Modifier.align(Alignment.Center),
            horizontalArrangement = Arrangement.spacedBy(40.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = { onSeekBy(-seekStep) }, modifier = Modifier.size(56.dp)) {
                CineoIcon(back, tint = Color.White, size = 36.sp, label = pluralStringResource(R.plurals.seek_back, seekStep, seekStep))
            }
            IconButton(
                onClick = onTogglePause,
                modifier = Modifier.size(76.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.15f)),
            ) {
                CineoIcon(
                    if (status.paused) Icon.PLAYER_PLAY else Icon.PLAYER_PAUSE,
                    filled = true,
                    tint = Color.White,
                    size = 40.sp,
                    label = stringResource(if (status.paused) R.string.play else R.string.pause),
                )
            }
            IconButton(onClick = { onSeekBy(seekStep) }, modifier = Modifier.size(56.dp)) {
                CineoIcon(forward, tint = Color.White, size = 36.sp, label = pluralStringResource(R.plurals.seek_forward, seekStep, seekStep))
            }
        }
        Column(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().safeDrawingPadding().padding(horizontal = 16.dp, vertical = 8.dp),
        ) {
            val duration = status.durationS.toFloat().coerceAtLeast(0f)
            SeekBar(
                position = position,
                duration = duration,
                onDrag = onSeekDrag,
                onDone = onSeekDone,
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(clock(position.toDouble()), color = Color.White, style = MaterialTheme.typography.labelMedium)
                Text(clock(status.durationS), color = Color.White.copy(alpha = 0.7f), style = MaterialTheme.typography.labelMedium)
            }
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

@Composable
private fun SeekBar(position: Float, duration: Float, onDrag: (Float) -> Unit, onDone: () -> Unit) {
    var width by remember { mutableStateOf(1) }
    val fraction = if (duration > 0f) (position / duration).coerceIn(0f, 1f) else 0f
    val at = { x: Float -> (x / width).coerceIn(0f, 1f) * duration }
    Box(
        Modifier
            .fillMaxWidth()
            .height(32.dp)
            .onSizeChanged { width = it.width.coerceAtLeast(1) }
            .pointerInput(duration) {
                if (duration <= 0f) return@pointerInput
                detectTapGestures { offset ->
                    onDrag(at(offset.x))
                    onDone()
                }
            }
            .pointerInput(duration) {
                if (duration <= 0f) return@pointerInput
                detectHorizontalDragGestures(
                    onDragStart = { onDrag(at(it.x)) },
                    onDragEnd = onDone,
                    onDragCancel = onDone,
                ) { change, _ -> onDrag(at(change.position.x)) }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        Box(Modifier.fillMaxWidth().height(4.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.25f)))
        Box(Modifier.fillMaxWidth(fraction).height(4.dp).clip(CircleShape).background(Brand.Accent))
        Box(
            Modifier
                .offset { IntOffset((fraction * width).roundToInt() - 7.dp.roundToPx(), 0) }
                .size(14.dp)
                .clip(CircleShape)
                .background(Brand.Accent),
        )
    }
}
