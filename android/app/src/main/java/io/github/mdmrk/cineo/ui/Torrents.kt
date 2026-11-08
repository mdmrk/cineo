package io.github.mdmrk.cineo.ui

import android.text.format.Formatter
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.ArtView
import io.github.mdmrk.cineo.rust.TorrentView

@Composable
fun TorrentStats(torrent: TorrentView?, modifier: Modifier = Modifier) {
    val streaming = torrent as? TorrentView.Streaming ?: return
    val context = LocalContext.current
    val size = { bytes: ULong -> Formatter.formatShortFileSize(context, bytes.toLong()) }
    Row(
        modifier
            .clip(RoundedCornerShape(50))
            .background(Color.Black.copy(alpha = 0.55f))
            .padding(horizontal = 10.dp, vertical = 5.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Stat(Icon.ARROW_DOWN, "${size(streaming.downloadBytesPerSec)}/s", stringResource(R.string.stats_download))
        Stat(Icon.USERS, streaming.peers.toString(), stringResource(R.string.stats_peers))
        Stat(
            Icon.DOWNLOAD,
            stringResource(R.string.stats_of, size(streaming.downloaded), size(streaming.size)),
            stringResource(R.string.stats_downloaded),
        )
    }
}

@Composable
private fun Stat(icon: Icon, value: String, label: String) {
    Row(
        Modifier.semantics(mergeDescendants = true) { contentDescription = "$label $value" },
        horizontalArrangement = Arrangement.spacedBy(3.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CineoIcon(icon, tint = Color.White.copy(alpha = 0.7f), size = 13.sp)
        Text(value, color = Color.White, style = MaterialTheme.typography.labelSmall, maxLines = 1)
    }
}

@Composable
fun ConnectingScreen(art: ArtView, torrent: TorrentView?, images: Images, onBack: () -> Unit) {
    BackHandler(onBack = onBack)
    Box(Modifier.fillMaxSize().background(Color.Black)) {
        LoadingBackdrop(art, images, firstLoad = true)
        Box(Modifier.fillMaxSize().safeDrawingPadding().padding(16.dp)) {
            IconButton(onClick = onBack, modifier = Modifier.align(Alignment.TopStart)) {
                CineoIcon(Icon.ARROW_LEFT, tint = Color.White, label = stringResource(R.string.back))
            }
            TorrentStats(torrent, Modifier.align(Alignment.TopEnd).padding(top = 10.dp))
        }
    }
}

@Composable
fun P2pPrompt(onAccept: () -> Unit, onDecline: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDecline,
        title = { Text(stringResource(R.string.p2p_title)) },
        text = { Text(stringResource(R.string.p2p_notice)) },
        confirmButton = { TextButton(onClick = onAccept) { Text(stringResource(R.string.p2p_accept)) } },
        dismissButton = { TextButton(onClick = onDecline) { Text(stringResource(R.string.cancel)) } },
    )
}
