package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.NowPlayingView
import io.github.mdmrk.cineo.rust.RemoteCommandView
import io.github.mdmrk.cineo.rust.RemoteFailure
import io.github.mdmrk.cineo.rust.RemotePeerView
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private const val BAR_POLL_MS = 2000L
private val BarShape = RoundedCornerShape(12.dp)

@Composable
fun RemoteBar(peer: RemotePeerView?, cineo: Cineo, onOpen: () -> Unit) {
    if (peer == null) return
    val context = LocalContext.current
    var playing by remember(peer) { mutableStateOf<NowPlayingView?>(null) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(peer) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            while (true) {
                playing = if (localNetworkAllowed(context)) {
                    try {
                        cineo.remoteSend(null).playing
                    } catch (e: RemoteFailure) {
                        null
                    }
                } else {
                    null
                }
                delay(BAR_POLL_MS)
            }
        }
    }
    val now = playing ?: return
    val scope = rememberCoroutineScope()
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 6.dp)
            .clip(BarShape)
            .background(Brand.Surface)
            .clickable(onClick = onOpen),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(start = 14.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            CineoIcon(Icon.DEVICE_DESKTOP, tint = Brand.Accent, size = 22.sp)
            Column(Modifier.weight(1f)) {
                Text(
                    now.title,
                    color = Brand.TextBright,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    stringResource(if (now.loading) R.string.remote_bar_loading else R.string.remote_bar_playing, peer.name),
                    color = Brand.TextDim,
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 1,
                )
            }
            IconButton(onClick = {
                playing = now.copy(paused = !now.paused)
                scope.launch {
                    try {
                        playing = cineo.remoteSend(RemoteCommandView.Pause(!now.paused)).playing?.copy(paused = !now.paused)
                    } catch (e: RemoteFailure) {
                        playing = now
                    }
                }
            }) {
                CineoIcon(
                    if (now.paused) Icon.PLAYER_PLAY else Icon.PLAYER_PAUSE,
                    filled = true,
                    tint = Brand.TextBright,
                    label = stringResource(if (now.paused) R.string.play else R.string.pause),
                )
            }
        }
        val fraction = if (now.durationS > 0) (now.positionS / now.durationS).toFloat().coerceIn(0f, 1f) else 0f
        Box(Modifier.fillMaxWidth().height(2.dp).background(Brand.Rule)) {
            Box(Modifier.fillMaxWidth(fraction).height(2.dp).background(Brand.Accent))
        }
    }
}
