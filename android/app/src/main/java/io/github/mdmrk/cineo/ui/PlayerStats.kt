package io.github.mdmrk.cineo.ui

import android.text.format.Formatter
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.DecoderView
import io.github.mdmrk.cineo.rust.PlayerStatsView
import io.github.mdmrk.cineo.rust.TorrentView

@Composable
fun StatsPanel(torrent: TorrentView?, stats: PlayerStatsView, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val size = { bytes: Long -> Formatter.formatShortFileSize(context, bytes) }
    val rows = buildList {
        if (torrent is TorrentView.Streaming) {
            add(stringResource(R.string.stats_download) to "${size(torrent.downloadBytesPerSec.toLong())}/s")
            add(stringResource(R.string.stats_peers) to torrent.peers.toString())
            add(
                stringResource(R.string.stats_downloaded) to
                    stringResource(R.string.stats_of, size(torrent.downloaded.toLong()), size(torrent.size.toLong())),
            )
        }
        stats.networkBytesPerSec?.let { add(stringResource(R.string.stats_network) to "${size(it.toLong())}/s") }
        stats.bufferedS?.let { add(stringResource(R.string.stats_buffered) to "%.0f s".format(it)) }
        stats.resolution?.let { add(stringResource(R.string.stats_resolution) to "${it.width}×${it.height}") }
        val video = listOfNotNull(stats.videoCodec, stats.videoBitsPerSec?.let { "%.1f Mb/s".format(it / 1_000_000) })
        if (video.isNotEmpty()) add(stringResource(R.string.stats_video) to video.joinToString(" · "))
        stats.decoder?.let {
            val name = when (it) {
                is DecoderView.Software -> stringResource(R.string.stats_software)
                is DecoderView.Hardware -> it.name
            }
            add(stringResource(R.string.stats_decoder) to name)
        }
        stats.droppedFrames?.let { add(stringResource(R.string.stats_dropped) to it.toString()) }
    }
    if (rows.isEmpty()) return
    Column(
        modifier
            .clip(RoundedCornerShape(8.dp))
            .background(Color.Black.copy(alpha = 0.67f))
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        rows.forEach { (label, value) ->
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Text(
                    label,
                    color = Color.White.copy(alpha = 0.6f),
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.width(110.dp),
                )
                Text(value, color = Color.White, style = MaterialTheme.typography.labelMedium, maxLines = 1)
            }
        }
    }
}
