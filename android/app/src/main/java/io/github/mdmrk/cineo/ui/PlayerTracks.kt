package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.PlayerStatusView
import io.github.mdmrk.cineo.rust.TrackView

private class Choice(val label: String, val selected: Boolean, val pick: (() -> Unit)?)

@Composable
fun SubtitlesMenu(
    status: PlayerStatusView,
    onTrack: (Long?) -> Unit,
    onAddon: (String) -> Unit,
    onOpen: (Boolean) -> Unit,
) {
    val choices = buildList {
        add(Choice(stringResource(R.string.subtitles_off), status.subtitlesOff) { onTrack(null) })
        status.subtitles.forEach { add(Choice(trackLabel(it), it.selected) { onTrack(it.id) }) }
        status.addonSubtitles.forEach { add(Choice(it.label, it.selected) { onAddon(it.url) }) }
    }
    TrackMenu(Icon.BADGE_CC, stringResource(R.string.player_subtitles), choices, onOpen)
}

@Composable
fun AudioMenu(status: PlayerStatusView, onTrack: (Long) -> Unit, onOpen: (Boolean) -> Unit) {
    val choices = status.audio.map { Choice(trackLabel(it), it.selected) { onTrack(it.id) } }
        .ifEmpty { listOf(Choice(stringResource(R.string.no_audio_tracks), false, null)) }
    TrackMenu(Icon.LANGUAGE, stringResource(R.string.player_audio), choices, onOpen)
}

@Composable
private fun trackLabel(track: TrackView): String =
    track.label ?: stringResource(R.string.track, track.number.toInt())

@Composable
private fun TrackMenu(icon: Icon, label: String, choices: List<Choice>, onOpen: (Boolean) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val show = { value: Boolean ->
        open = value
        onOpen(value)
    }
    Box {
        IconButton(onClick = { show(true) }, modifier = Modifier.semantics { contentDescription = label }) {
            CineoIcon(icon, tint = Color.White)
        }
        DropdownMenu(expanded = open, onDismissRequest = { show(false) }) {
            choices.forEach { choice ->
                DropdownMenuItem(
                    text = { Text(choice.label, maxLines = 1) },
                    enabled = choice.pick != null,
                    leadingIcon = {
                        Box(Modifier.size(20.dp)) {
                            if (choice.selected) CineoIcon(Icon.CHECK, tint = Brand.Accent, size = 20.sp)
                        }
                    },
                    onClick = {
                        show(false)
                        choice.pick?.invoke()
                    },
                )
            }
        }
    }
}
