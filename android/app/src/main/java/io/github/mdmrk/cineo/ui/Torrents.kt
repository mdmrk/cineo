package io.github.mdmrk.cineo.ui

import android.text.format.Formatter
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
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
import io.github.mdmrk.cineo.rust.TorrentView

@Composable
fun TorrentStats(torrent: TorrentView?) {
    val streaming = torrent as? TorrentView.Streaming ?: return
    val context = LocalContext.current
    val size = { bytes: ULong -> Formatter.formatShortFileSize(context, bytes.toLong()) }
    val rows = listOf(
        stringResource(R.string.stats_download) to "${size(streaming.downloadBytesPerSec)}/s",
        stringResource(R.string.stats_peers) to streaming.peers.toString(),
        stringResource(R.string.stats_downloaded) to
            stringResource(R.string.stats_of, size(streaming.downloaded), size(streaming.size)),
    )
    Text(
        rows.joinToString("  ·  ") { (name, value) -> "$name $value" },
        color = Color.White,
        style = MaterialTheme.typography.bodySmall,
    )
}

@Composable
fun ConnectingScreen(title: String, torrent: TorrentView?, onBack: () -> Unit) {
    BackHandler(onBack = onBack)
    Box(Modifier.fillMaxSize().background(Color.Black).safeDrawingPadding().padding(16.dp)) {
        TextButton(onClick = onBack, modifier = Modifier.align(Alignment.TopStart)) {
            Text(stringResource(R.string.back), color = Color.White)
        }
        Column(
            modifier = Modifier.align(Alignment.Center),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            CircularProgressIndicator()
            Text(title, color = Color.White, style = MaterialTheme.typography.titleMedium)
            TorrentStats(torrent)
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
