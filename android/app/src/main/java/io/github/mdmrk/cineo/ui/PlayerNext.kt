package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.NextView
import io.github.mdmrk.cineo.rust.PlayerStatusView

@Composable
internal fun NextEpisode(
    next: NextView,
    status: PlayerStatusView,
    noticeS: Int,
    auto: Boolean,
    images: Images,
    onPlay: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var dismissed by remember(next.title) { mutableStateOf(false) }
    val notice = remember(next.title) { NoticeClock() }
    var requested by remember(next.title) { mutableStateOf(false) }
    val remaining = status.durationS - status.positionS
    val inNotice = status.loaded && status.durationS > 0.0 && remaining > 0.0 && remaining <= noticeS
    val since = if (inNotice) notice.shownAt?.takeIf { it <= status.positionS } ?: status.positionS else null
    SideEffect { notice.shownAt = since }
    if (since == null || dismissed) return
    val left = (AUTO_NEXT_S - (status.positionS - since)).coerceAtLeast(0.0)
    LaunchedEffect(auto, left <= 0.0) {
        if (auto && left <= 0.0 && !requested) {
            requested = true
            onPlay()
        }
    }
    Box(
        modifier
            .width(280.dp)
            .height(158.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(Brand.Panel)
            .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {},
    ) {
        NetImage(images, next.image, 280.dp, Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.6f)))
        IconButton(onClick = { dismissed = true }, modifier = Modifier.align(Alignment.TopEnd)) {
            CineoIcon(Icon.X, tint = Color.White, label = stringResource(R.string.dismiss))
        }
        Column(
            Modifier.align(Alignment.Center).padding(horizontal = 16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                if (auto) stringResource(R.string.next_episode_in, kotlin.math.ceil(left).toInt())
                else stringResource(R.string.next_episode),
                color = Brand.Text,
                style = MaterialTheme.typography.labelMedium,
            )
            Text(
                next.title,
                color = Brand.TextBright,
                style = MaterialTheme.typography.titleSmall,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Button(
                onClick = {
                    requested = true
                    onPlay()
                },
                colors = ButtonDefaults.buttonColors(containerColor = Brand.Accent, contentColor = Brand.OnAccent),
            ) {
                CineoIcon(Icon.PLAYER_PLAY, filled = true, tint = Brand.OnAccent)
                Text(stringResource(R.string.play), modifier = Modifier.padding(start = 6.dp))
            }
        }
    }
}

private const val AUTO_NEXT_S = 10.0

private class NoticeClock {
    var shownAt: Double? = null
}
