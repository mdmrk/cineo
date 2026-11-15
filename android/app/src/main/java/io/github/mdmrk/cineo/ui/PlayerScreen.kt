package io.github.mdmrk.cineo.ui

import android.content.pm.ActivityInfo
import android.os.SystemClock
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalActivity
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.compose.LifecycleStartEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mdmrk.cineo.Native
import io.github.mdmrk.cineo.Player
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.PlaybackView
import io.github.mdmrk.cineo.rust.TorrentView
import kotlinx.coroutines.delay

private const val FAST_SPEED = 2.0
private const val BUFFERING_AFTER_MS = 400L

@Composable
fun PlayerScreen(
    playback: PlaybackView,
    torrent: TorrentView?,
    images: Images,
    cineo: Cineo,
    player: Player,
    inPip: Boolean,
    pipOnLeave: Boolean,
    onPip: (Pip) -> Unit,
) {
    val status by player.status.collectAsStateWithLifecycle()
    val current = status
    var controls by remember { mutableStateOf(true) }
    var touches by remember { mutableIntStateOf(0) }
    var buffering by remember { mutableStateOf(false) }
    var hold by remember { mutableStateOf(SeekHold()) }
    var stats by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var fast by remember { mutableStateOf(false) }
    val started = current?.started == true
    val paused = current?.paused == true
    val waiting = current?.waiting == true

    LaunchedEffect(current) {
        if (current != null) hold = hold.settle(current.positionS, waiting, SystemClock.elapsedRealtime())
    }
    LaunchedEffect(started && waiting) {
        buffering = false
        if (started && waiting) {
            delay(BUFFERING_AFTER_MS)
            buffering = true
        }
    }
    LaunchedEffect(paused) {
        if (paused) controls = true
    }
    LifecycleStartEffect(Unit) {
        controls = true
        onStopOrDispose {}
    }
    LaunchedEffect(controls, touches, paused, menu, started, hold.drag != null) {
        if (controls && started && !paused && !menu && hold.drag == null) {
            delay(playback.hideControlsMs.toLong())
            controls = false
        }
    }

    val size = current?.stats?.resolution
    val pip = Pip(
        auto = pipOnLeave && started && !paused,
        width = size?.width?.toInt() ?: 0,
        height = size?.height?.toInt() ?: 0,
        paused = paused,
        step = playback.seekStepS.toInt(),
    )
    LaunchedEffect(pip) { onPip(pip) }
    DisposableEffect(Unit) {
        onDispose {
            onPip(Pip.Off)
            if (fast) player.speed(1.0)
        }
    }

    BackHandler { player.stop() }
    FullScreen()
    val onRotate = rememberRotate()
    val haptics = LocalHapticFeedback.current
    val canSpeedUp by rememberUpdatedState(started && !paused)
    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .pointerInput(Unit) {
                detectTapGestures(
                    onPress = {
                        tryAwaitRelease()
                        if (fast) {
                            fast = false
                            player.speed(1.0)
                        }
                    },
                    onLongPress = {
                        if (canSpeedUp) {
                            fast = true
                            controls = false
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            player.speed(FAST_SPEED)
                        }
                    },
                    onTap = { controls = !controls },
                )
            },
    ) {
        AndroidView(
            factory = { context ->
                SurfaceView(context).apply {
                    holder.addCallback(SurfaceCallback(cineo))
                }
            },
            update = { it.keepScreenOn = !paused },
            modifier = Modifier.fillMaxSize(),
        )
        when {
            !started && inPip -> Box(Modifier.fillMaxSize().background(Color.Black)) {
                LoadingBackdrop(playback.art, images, firstLoad = true)
            }
            !started -> LoadingWithBack(playback.art, images, onBack = player::stop)
            buffering -> Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.45f))) {
                LoadingBackdrop(playback.art, images, firstLoad = false)
            }
        }
        if (inPip || !started || current == null) return@Box
        AnimatedVisibility(controls, enter = fadeIn(), exit = fadeOut()) {
            Controls(
                title = playback.art.title,
                status = current,
                position = (hold.value ?: current.positionS).toFloat(),
                onSeekDrag = { hold = hold.dragTo(it.toDouble()) },
                onSeekDone = {
                    hold.drag?.let {
                        hold = hold.commit(it, SystemClock.elapsedRealtime())
                        player.seekTo(it)
                    }
                },
                onBack = player::stop,
                onTogglePause = {
                    touches++
                    player.togglePause()
                },
                seekStep = playback.seekStepS.toInt(),
                onSeekBy = {
                    touches++
                    hold = hold.commitBy(it.toDouble(), current.positionS, current.durationS, SystemClock.elapsedRealtime())
                    player.seekBy(it.toDouble())
                },
                onNext = playback.next?.let { { player.next() } },
                onToggleStats = {
                    touches++
                    stats = !stats
                },
                onRotate = {
                    touches++
                    onRotate()
                },
                onAudio = { cineo.selectAudio(it) },
                onSubtitle = { cineo.selectSubtitle(it) },
                onAddonSubtitle = { cineo.selectAddonSubtitle(it) },
                onMenu = { menu = it },
            )
        }
        if (fast) {
            FastBadge(Modifier.align(Alignment.TopCenter).safeDrawingPadding().padding(top = 16.dp))
        }
        val next = playback.next
        if (next != null) {
            NextEpisode(
                next = next,
                status = current,
                noticeS = playback.nextNoticeS.toInt(),
                auto = playback.bingeWatching,
                images = images,
                onPlay = player::next,
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .safeDrawingPadding()
                    .padding(end = 16.dp, bottom = 120.dp),
            )
        }
        if (stats) {
            StatsPanel(
                torrent,
                current.stats,
                Modifier.align(Alignment.TopStart).safeDrawingPadding().padding(start = 16.dp, top = 72.dp),
            )
        }
    }
}

@Composable
private fun FastBadge(modifier: Modifier = Modifier) {
    Row(
        modifier
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.55f))
            .padding(horizontal = 14.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("2×", color = Color.White, style = MaterialTheme.typography.labelLarge)
        Row {
            CineoIcon(Icon.PLAYER_PLAY, filled = true, tint = Color.White, size = 12.sp)
            CineoIcon(Icon.PLAYER_PLAY, filled = true, tint = Color.White, size = 12.sp)
        }
    }
}

@Composable
private fun FullScreen() {
    val view = LocalView.current
    val activity = LocalActivity.current
    DisposableEffect(view) {
        val window = activity?.window
        val insets = window?.let { WindowCompat.getInsetsController(it, view) }
        insets?.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        insets?.hide(WindowInsetsCompat.Type.systemBars())
        onDispose { insets?.show(WindowInsetsCompat.Type.systemBars()) }
    }
}

@Composable
private fun rememberRotate(): () -> Unit {
    val view = LocalView.current
    val activity = LocalActivity.current
    DisposableEffect(activity) {
        onDispose { activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED }
    }
    return { activity?.requestedOrientation = rotatedOrientation(view.resources.configuration.orientation) }
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
