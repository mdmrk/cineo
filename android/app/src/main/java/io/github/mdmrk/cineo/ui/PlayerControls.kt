package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.CachedSpanView
import io.github.mdmrk.cineo.rust.PlayerStatusView
import kotlin.math.roundToInt

@Composable
internal fun Controls(
    title: String,
    status: PlayerStatusView,
    position: Float,
    onSeekDrag: (Float) -> Unit,
    onSeekDone: () -> Unit,
    onBack: () -> Unit,
    onTogglePause: () -> Unit,
    seekStep: Int,
    onSeekBy: (Int) -> Unit,
    onNext: (() -> Unit)?,
    onToggleStats: () -> Unit,
    onRotate: (() -> Unit)?,
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
                style = MaterialTheme.typography.titleSmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            SubtitlesMenu(status, onTrack = onSubtitle, onAddon = onAddonSubtitle, onOpen = onMenu)
            AudioMenu(status, onTrack = onAudio, onOpen = onMenu)
            val statsLabel = stringResource(R.string.player_stats)
            IconButton(onClick = onToggleStats, modifier = Modifier.semantics { contentDescription = statsLabel }) {
                CineoIcon(Icon.ACTIVITY, tint = Color.White)
            }
            if (onRotate != null) {
                IconButton(onClick = onRotate) {
                    CineoIcon(Icon.DEVICE_MOBILE_ROTATED, tint = Color.White, label = stringResource(R.string.player_rotate))
                }
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
            if (onNext != null) {
                Row(
                    Modifier
                        .align(Alignment.End)
                        .clip(CircleShape)
                        .clickable(onClick = onNext)
                        .padding(horizontal = 10.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    CineoIcon(Icon.PLAYER_SKIP_FORWARD, filled = true, tint = Color.White, size = 16.sp)
                    Text(
                        stringResource(R.string.next_episode),
                        color = Color.White,
                        style = MaterialTheme.typography.labelMedium,
                        modifier = Modifier.padding(start = 6.dp),
                    )
                }
            }
            val duration = status.durationS.toFloat().coerceAtLeast(0f)
            SeekBar(
                position = position,
                duration = duration,
                cached = status.cached,
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

internal fun clock(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val (h, m, s) = Triple(total / 3600, total / 60 % 60, total % 60)
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}

@Composable
internal fun SeekBar(position: Float, duration: Float, cached: List<CachedSpanView>, onDrag: (Float) -> Unit, onDone: () -> Unit) {
    var width by remember { mutableIntStateOf(1) }
    val fraction = if (duration > 0f) (position / duration).coerceIn(0f, 1f) else 0f
    val latestDuration by rememberUpdatedState(duration)
    val seekable = duration > 0f
    val at = { x: Float -> (x / width).coerceIn(0f, 1f) * latestDuration }
    Box(
        Modifier
            .fillMaxWidth()
            .height(32.dp)
            .onSizeChanged { width = it.width.coerceAtLeast(1) }
            .pointerInput(seekable) {
                if (!seekable) return@pointerInput
                detectTapGestures { offset ->
                    onDrag(at(offset.x))
                    onDone()
                }
            }
            .pointerInput(seekable) {
                if (!seekable) return@pointerInput
                detectHorizontalDragGestures(
                    onDragStart = { onDrag(at(it.x)) },
                    onDragEnd = onDone,
                    onDragCancel = onDone,
                ) { change, _ -> onDrag(at(change.position.x)) }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        Canvas(Modifier.fillMaxWidth().height(2.dp).offset(y = 5.dp)) {
            cached.forEach {
                drawRect(
                    Color.White.copy(alpha = 0.6f),
                    topLeft = Offset(it.start * size.width, 0f),
                    size = Size((it.end - it.start) * size.width, size.height),
                )
            }
        }
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
