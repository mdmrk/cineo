package io.github.mdmrk.cineo.ui

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.LocalActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.SliderState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.app.ActivityCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.NowPlayingView
import io.github.mdmrk.cineo.rust.RemoteCommandView
import io.github.mdmrk.cineo.rust.RemoteFailure
import io.github.mdmrk.cineo.rust.RemotePeerView
import io.github.mdmrk.cineo.rust.RemoteStatusView
import io.github.mdmrk.cineo.rust.RemoteTrackView
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

private const val POLL_MS = 1000L
private const val SETTLE_MS = 400L
private const val SEEK_STEP_S = 10.0
private const val DELAY_STEP_S = 0.1
private val SPEEDS = listOf(0.5, 0.75, 1.0, 1.25, 1.5, 2.0)

@Composable
fun RemoteScreen(peer: RemotePeerView, cineo: Cineo, onBack: () -> Unit) {
    val context = LocalContext.current
    var allowed by remember { mutableStateOf(localNetworkAllowed(context)) }
    LifecycleResumeEffect(Unit) {
        allowed = localNetworkAllowed(context)
        onPauseOrDispose {}
    }
    var status by remember { mutableStateOf<RemoteStatusView?>(null) }
    var failure by remember { mutableStateOf<RemoteFailure?>(null) }
    val scope = rememberCoroutineScope()
    val refresh: suspend (RemoteCommandView?) -> Unit = { command ->
        try {
            status = cineo.remoteSend(command)
            failure = null
        } catch (e: RemoteFailure) {
            failure = e
        }
    }
    val send = { command: RemoteCommandView, after: () -> Unit ->
        scope.launch {
            refresh(command)
            delay(SETTLE_MS)
            refresh(null)
            after()
        }
        Unit
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(allowed, peer) {
        if (!allowed) return@LaunchedEffect
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            while (true) {
                refresh(null)
                delay(POLL_MS)
            }
        }
    }
    Column(Modifier.fillMaxSize().background(Brand.Background).safeDrawingPadding()) {
        Row(
            Modifier.fillMaxWidth().padding(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onBack) { CineoIcon(Icon.ARROW_LEFT, tint = Brand.TextBright, label = stringResource(R.string.back)) }
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.section_remote), color = Brand.TextBright, style = MaterialTheme.typography.titleMedium)
                Text(peer.address, color = Brand.TextDim, style = MaterialTheme.typography.bodySmall)
            }
        }
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            val playing = status?.playing
            when {
                !allowed -> LocalNetworkNeeded { allowed = it }
                failure != null && status == null -> Message(Icon.ALERT_CIRCLE, failureText(failure!!))
                status == null -> Message(Icon.DEVICE_DESKTOP, stringResource(R.string.remote_connecting))
                playing == null -> {
                    failure?.let { Warning(failureText(it)) }
                    Message(Icon.DEVICE_DESKTOP, stringResource(R.string.remote_nothing))
                }
                else -> {
                    failure?.let { Warning(failureText(it)) }
                    Playing(playing, send)
                }
            }
        }
    }
}

@Composable
private fun Message(icon: Icon, text: String) {
    Column(
        Modifier.fillMaxWidth().padding(top = 64.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        CineoIcon(icon, tint = Brand.TextDim, size = 56.sp)
        Text(text, color = Brand.TextDim, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
    }
}

@Composable
private fun Warning(text: String) {
    Text(text, color = Brand.Warning, style = MaterialTheme.typography.bodySmall, textAlign = TextAlign.Center)
}

@Composable
private fun LocalNetworkNeeded(onAllowed: (Boolean) -> Unit) {
    val context = LocalContext.current
    val activity = LocalActivity.current
    var refused by remember { mutableStateOf(false) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        refused = !granted
        onAllowed(granted)
    }
    LaunchedEffect(Unit) { ask.launch(LOCAL_NETWORK) }
    Message(Icon.WORLD, stringResource(R.string.remote_local_network))
    TextButton(onClick = {
        if (refused && activity != null && !ActivityCompat.shouldShowRequestPermissionRationale(activity, LOCAL_NETWORK)) {
            context.startActivity(
                Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null)),
            )
        } else {
            ask.launch(LOCAL_NETWORK)
        }
    }) { Text(stringResource(R.string.private_network_allow), color = Brand.Accent) }
}

@Composable
private fun failureText(failure: RemoteFailure): String = when (failure) {
    is RemoteFailure.Rejected -> stringResource(R.string.remote_failed_rejected, failure.status.toInt())
    else -> stringResource(
        when (failure) {
            is RemoteFailure.WrongKey -> R.string.remote_failed_key
            is RemoteFailure.Clock -> R.string.remote_failed_clock
            is RemoteFailure.Unreadable, is RemoteFailure.Invalid -> R.string.remote_failed_unreadable
            else -> R.string.remote_failed_unreachable
        },
    )
}

@Composable
private fun Playing(playing: NowPlayingView, send: (RemoteCommandView, () -> Unit) -> Unit) {
    var seeking by remember { mutableStateOf<Float?>(null) }
    var volume by remember { mutableStateOf<Float?>(null) }
    val step = SEEK_STEP_S.toInt()
    Text(
        playing.title,
        color = Brand.TextBright,
        style = MaterialTheme.typography.titleLarge,
        textAlign = TextAlign.Center,
        maxLines = 3,
        overflow = TextOverflow.Ellipsis,
    )
    if (playing.loading) {
        Text(stringResource(R.string.remote_loading), color = Brand.TextDim, style = MaterialTheme.typography.bodyMedium)
    }
    Column(Modifier.fillMaxWidth().widthIn(max = 560.dp)) {
        val position = seeking ?: playing.positionS.toFloat()
        SeekBar(
            position = position,
            duration = playing.durationS.toFloat().coerceAtLeast(0f),
            cached = emptyList(),
            onDrag = { seeking = it },
            onDone = {
                seeking?.let { target -> send(RemoteCommandView.SeekTo(target.toDouble())) { seeking = null } }
            },
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(clock(position.toDouble()), color = Brand.TextBright, style = MaterialTheme.typography.labelMedium)
            Text(clock(playing.durationS), color = Brand.TextDim, style = MaterialTheme.typography.labelMedium)
        }
    }
    Row(horizontalArrangement = Arrangement.spacedBy(32.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = { send(RemoteCommandView.SeekBy(-SEEK_STEP_S)) {} }, modifier = Modifier.size(56.dp)) {
            CineoIcon(Icon.REWIND_BACKWARD_10, tint = Brand.TextBright, size = 36.sp, label = pluralStringResource(R.plurals.seek_back, step, step))
        }
        IconButton(
            onClick = { send(RemoteCommandView.Pause(!playing.paused)) {} },
            modifier = Modifier.size(80.dp).clip(CircleShape).background(Brand.Accent),
        ) {
            CineoIcon(
                if (playing.paused) Icon.PLAYER_PLAY else Icon.PLAYER_PAUSE,
                filled = true,
                tint = Brand.OnAccent,
                size = 40.sp,
                label = stringResource(if (playing.paused) R.string.play else R.string.pause),
            )
        }
        IconButton(onClick = { send(RemoteCommandView.SeekBy(SEEK_STEP_S)) {} }, modifier = Modifier.size(56.dp)) {
            CineoIcon(Icon.REWIND_FORWARD_10, tint = Brand.TextBright, size = 36.sp, label = pluralStringResource(R.plurals.seek_forward, step, step))
        }
    }
    Row(
        Modifier.fillMaxWidth().widthIn(max = 560.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val muted = playing.muted
        IconButton(onClick = { send(RemoteCommandView.Mute(!muted)) {} }) {
            CineoIcon(
                if (muted || playing.volume <= 0.0) Icon.VOLUME_OFF else Icon.VOLUME,
                tint = Brand.TextBright,
                label = stringResource(if (muted) R.string.remote_unmute else R.string.remote_mute),
            )
        }
        val slider = remember { SliderState(0f, trackRange = 0f..100f) }
        slider.value = volume ?: if (muted) 0f else playing.volume.toFloat()
        Slider(
            state = slider,
            onValueChange = { volume = it },
            onValueChangeFinished = {
                volume?.let { target -> send(RemoteCommandView.Volume(target.roundToInt().toDouble())) { volume = null } }
            },
            modifier = Modifier.weight(1f),
            colors = SliderDefaults.colors(thumbColor = Brand.Accent, activeTrackColor = Brand.Accent),
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        TrackPicker(
            label = stringResource(R.string.player_audio),
            icon = Icon.LANGUAGE,
            tracks = playing.audio,
            offLabel = null,
            onPick = { send(RemoteCommandView.Audio(it)) {} },
        )
        TrackPicker(
            label = stringResource(R.string.player_subtitles),
            icon = Icon.BADGE_CC,
            tracks = playing.subtitles,
            offLabel = stringResource(R.string.subtitles_off),
            onPick = { send(RemoteCommandView.Subtitle(it)) {} },
        )
        SpeedPicker(playing.speed) { send(RemoteCommandView.Speed(it)) {} }
    }
    SubtitleDelay(playing.subtitleDelayS) { send(RemoteCommandView.SubtitleDelay(it)) {} }
    Spacer(Modifier.height(8.dp))
    OutlinedButton(
        onClick = { send(RemoteCommandView.Stop) {} },
        border = BorderStroke(1.dp, Brand.Danger.copy(alpha = 0.6f)),
    ) {
        CineoIcon(Icon.PLAYER_STOP, filled = true, tint = Brand.Danger, size = 18.sp)
        Text(stringResource(R.string.remote_stop), color = Brand.Danger, modifier = Modifier.padding(start = 8.dp))
    }
}

@Composable
private fun TrackPicker(
    label: String,
    icon: Icon,
    tracks: List<RemoteTrackView>,
    offLabel: String?,
    onPick: (Long?) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val choices = buildList {
        if (offLabel != null) add(Triple(offLabel, tracks.none { it.selected }, null as Long?))
        tracks.forEach { track ->
            add(Triple(track.label ?: stringResource(R.string.track, track.number.toInt()), track.selected, track.id))
        }
    }
    Box {
        OutlinedButton(onClick = { open = true }, enabled = tracks.isNotEmpty()) {
            CineoIcon(icon, tint = Brand.TextBright, size = 18.sp)
            Text(label, color = Brand.TextBright, modifier = Modifier.padding(start = 6.dp))
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            choices.forEach { (text, selected, id) ->
                DropdownMenuItem(
                    text = { Text(text, maxLines = 1) },
                    leadingIcon = {
                        Box(Modifier.size(20.dp)) {
                            if (selected) CineoIcon(Icon.CHECK, tint = Brand.Accent, size = 20.sp)
                        }
                    },
                    onClick = {
                        open = false
                        onPick(id)
                    },
                )
            }
        }
    }
}

@Composable
private fun SpeedPicker(speed: Double, onPick: (Double) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val label = @Composable { value: Double ->
        if (value == 1.0) stringResource(R.string.remote_speed_normal) else "${value}×"
    }
    Box {
        OutlinedButton(onClick = { open = true }) {
            Text(label(speed), color = Brand.TextBright)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            SPEEDS.forEach { value ->
                DropdownMenuItem(
                    text = { Text(label(value)) },
                    leadingIcon = {
                        Box(Modifier.size(20.dp)) {
                            if (value == speed) CineoIcon(Icon.CHECK, tint = Brand.Accent, size = 20.sp)
                        }
                    },
                    onClick = {
                        open = false
                        onPick(value)
                    },
                )
            }
        }
    }
}

@Composable
private fun SubtitleDelay(delayS: Double, onSet: (Double) -> Unit) {
    val rounded = (delayS * 10).roundToInt() / 10.0
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(stringResource(R.string.remote_subtitle_delay), color = Brand.TextDim, style = MaterialTheme.typography.labelMedium)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = { onSet(rounded - DELAY_STEP_S) }) {
                Text("−0.1 s", color = Brand.TextBright)
            }
            TextButton(onClick = { onSet(0.0) }, enabled = rounded != 0.0) {
                Text("%+.1f s".format(rounded), color = Brand.TextBright)
            }
            TextButton(onClick = { onSet(rounded + DELAY_STEP_S) }) {
                Text("+0.1 s", color = Brand.TextBright)
            }
        }
    }
}
