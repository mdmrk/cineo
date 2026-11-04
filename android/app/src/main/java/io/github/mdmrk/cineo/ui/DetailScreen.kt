package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.DetailView
import io.github.mdmrk.cineo.rust.MetaStatus
import io.github.mdmrk.cineo.rust.SourceKindView
import io.github.mdmrk.cineo.rust.StreamItems
import io.github.mdmrk.cineo.rust.VideoView

@Composable
fun DetailScreen(
    detail: DetailView,
    images: Images,
    onSelectVideo: (String) -> Unit,
    onPlay: (UInt, UInt) -> Unit,
    onFavorite: (Boolean) -> Unit,
) {
    Scaffold { padding ->
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = padding.calculateBottomPadding() + 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            item(key = "header") { Header(detail, images, onFavorite) }
            if (detail.meta is MetaStatus.Failed) {
                item(key = "meta-failed") {
                    Text(
                        problemText((detail.meta as MetaStatus.Failed).problem),
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                }
            }
            if (detail.videos.size > 1) {
                item(key = "episodes") { Episodes(detail, onSelectVideo) }
            }
            item(key = "streams-title") {
                Text(
                    stringResource(R.string.where_to_watch),
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
            if (detail.streams.isEmpty()) {
                item(key = "no-streams") {
                    Text(stringResource(R.string.streams_unavailable), modifier = Modifier.padding(horizontal = 16.dp))
                }
            }
            detail.streams.forEachIndexed { groupIndex, group ->
                item(key = "group-$groupIndex") {
                    Text(
                        group.addonName,
                        style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                }
                when (val items = group.items) {
                    is StreamItems.Loading -> item(key = "loading-$groupIndex") {
                        Box(Modifier.fillMaxWidth().padding(16.dp)) {
                            CircularProgressIndicator(Modifier.align(Alignment.Center))
                        }
                    }
                    is StreamItems.Failed -> item(key = "failed-$groupIndex") {
                        Text(
                            problemText(items.problem),
                            color = MaterialTheme.colorScheme.error,
                            modifier = Modifier.padding(horizontal = 16.dp),
                        )
                    }
                    is StreamItems.Ready -> {
                        if (items.streams.isEmpty()) {
                            item(key = "empty-$groupIndex") {
                                Text(stringResource(R.string.streams_empty), modifier = Modifier.padding(horizontal = 16.dp))
                            }
                        }
                        itemsIndexed(items.streams, key = { index, _ -> "stream-$groupIndex-$index" }) { index, stream ->
                            val playable = stream.kind == SourceKindView.HTTP || stream.kind == SourceKindView.TORRENT
                            ListItem(
                                headlineContent = { Text(stream.name ?: group.addonName) },
                                supportingContent = {
                                    val text = if (playable) stream.description else unsupportedText(stream.kind)
                                    if (text != null) Text(text)
                                },
                                modifier = Modifier.clickable(enabled = playable) {
                                    onPlay(groupIndex.toUInt(), index.toUInt())
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun unsupportedText(kind: SourceKindView): String =
    noticeText(io.github.mdmrk.cineo.rust.NoticeView.UnsupportedSource(kind))

@Composable
private fun Header(detail: DetailView, images: Images, onFavorite: (Boolean) -> Unit) {
    val width = LocalConfiguration.current.screenWidthDp
    var background by remember(detail.background) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(detail.background) {
        background = detail.background?.let { images.load(it, width * 3) }
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f)) {
            background?.let {
                Image(it, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
            }
            if (detail.meta is MetaStatus.Loading) {
                CircularProgressIndicator(Modifier.align(Alignment.Center))
            }
        }
        Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(detail.name ?: detail.id, style = MaterialTheme.typography.headlineSmall)
            val facts = listOfNotNull(detail.releaseInfo, detail.runtime) + detail.genres
            if (facts.isNotEmpty()) {
                Text(facts.joinToString(" · "), style = MaterialTheme.typography.bodyMedium)
            }
            detail.description?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
            FilledTonalButton(onClick = { onFavorite(!detail.favorite) }) {
                Text(stringResource(if (detail.favorite) R.string.favourite_remove else R.string.favourite_add))
            }
        }
    }
}

@Composable
private fun Episodes(detail: DetailView, onSelectVideo: (String) -> Unit) {
    val seasons = detail.videos.mapNotNull { it.season?.toInt() }.distinct().sortedWith(compareBy({ it == 0 }, { it }))
    val selectedSeason = detail.videos.firstOrNull { it.id == detail.selectedVideo }?.season?.toInt()
    var season by rememberSaveable(detail.id) { mutableStateOf(selectedSeason ?: seasons.firstOrNull()) }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            stringResource(R.string.episodes),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
        if (seasons.size > 1) {
            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(seasons) { number ->
                    FilterChip(
                        selected = number == season,
                        onClick = { season = number },
                        label = {
                            Text(
                                if (number == 0) stringResource(R.string.specials)
                                else stringResource(R.string.season, number),
                            )
                        },
                    )
                }
            }
        }
        detail.videos.filter { season == null || it.season?.toInt() == season }.forEach { video ->
            Episode(video, video.id == detail.selectedVideo) { onSelectVideo(video.id) }
        }
    }
}

@Composable
private fun Episode(video: VideoView, selected: Boolean, onClick: () -> Unit) {
    val number = video.episode?.let { "$it. " }.orEmpty()
    ListItem(
        headlineContent = {
            Text(
                number + video.title,
                color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            )
        },
        supportingContent = video.released?.let { { Text(it) } },
        modifier = Modifier.clickable(onClick = onClick),
    )
}
