package io.github.mdmrk.cineo.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.DetailView
import io.github.mdmrk.cineo.rust.LibraryItemView
import io.github.mdmrk.cineo.rust.MetaStatus
import io.github.mdmrk.cineo.rust.NoticeView
import io.github.mdmrk.cineo.rust.SourceKindView
import io.github.mdmrk.cineo.rust.StreamItems
import io.github.mdmrk.cineo.rust.StreamView
import io.github.mdmrk.cineo.rust.VideoView

private val Rounded = RoundedCornerShape(8.dp)

@Composable
fun DetailScreen(
    detail: DetailView,
    resume: LibraryItemView?,
    images: Images,
    onBack: () -> Unit,
    onResume: () -> Unit,
    onSelectVideo: (String) -> Unit,
    onPlay: (UInt, UInt) -> Unit,
    onFavorite: (Boolean) -> Unit,
) {
    Box(Modifier.fillMaxSize().background(Brand.Background)) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            item(key = "header") { Header(detail, images) }
            item(key = "actions") { Actions(resume, onResume) }
            if (detail.meta is MetaStatus.Failed) {
                item(key = "meta-failed") {
                    Text(
                        problemText((detail.meta as MetaStatus.Failed).problem),
                        color = Brand.Danger,
                        modifier = Modifier.padding(horizontal = 20.dp),
                    )
                }
            }
            if (detail.videos.size > 1) {
                item(key = "episodes") { Episodes(detail, images, onSelectVideo) }
            }
            item(key = "streams-title") {
                RowHeader(stringResource(R.string.where_to_watch), null)
            }
            if (detail.streams.isEmpty()) {
                item(key = "no-streams") {
                    Text(
                        stringResource(R.string.streams_unavailable),
                        color = Brand.TextDim,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                }
            }
            detail.streams.forEachIndexed { groupIndex, group ->
                item(key = "group-$groupIndex") {
                    Text(
                        group.addonName,
                        style = MaterialTheme.typography.labelLarge,
                        color = Brand.TextDim,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                }
                when (val items = group.items) {
                    is StreamItems.Loading -> items(2, key = { "loading-$groupIndex-$it" }) {
                        Placeholder(Modifier.padding(horizontal = 16.dp).fillMaxWidth().height(64.dp).clip(Rounded))
                    }
                    is StreamItems.Failed -> item(key = "failed-$groupIndex") {
                        Text(
                            problemText(items.problem),
                            color = Brand.TextDim,
                            modifier = Modifier.padding(horizontal = 16.dp),
                        )
                    }
                    is StreamItems.Ready -> {
                        if (items.streams.isEmpty()) {
                            item(key = "empty-$groupIndex") {
                                Text(
                                    stringResource(R.string.streams_empty),
                                    color = Brand.TextDim,
                                    modifier = Modifier.padding(horizontal = 16.dp),
                                )
                            }
                        }
                        itemsIndexed(items.streams, key = { index, _ -> "stream-$groupIndex-$index" }) { index, stream ->
                            StreamCard(stream, group.addonName) { onPlay(groupIndex.toUInt(), index.toUInt()) }
                        }
                    }
                }
            }
        }
        Row(
            Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 8.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            RoundButton(Icon.ARROW_LEFT, false, Brand.TextBright, onBack)
            RoundButton(Icon.HEART, detail.favorite, if (detail.favorite) Brand.Accent else Brand.TextBright) {
                onFavorite(!detail.favorite)
            }
        }
    }
}

@Composable
private fun RoundButton(icon: Icon, filled: Boolean, tint: Color, onClick: () -> Unit) {
    IconButton(
        onClick = onClick,
        modifier = Modifier.clip(CircleShape).background(Color.Black.copy(alpha = 0.45f)),
    ) {
        CineoIcon(icon, filled = filled, tint = tint)
    }
}

@Composable
private fun Header(detail: DetailView, images: Images) {
    val width = LocalConfiguration.current.screenWidthDp.dp
    var expanded by rememberSaveable(detail.id) { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.fillMaxWidth().aspectRatio(0.95f)) {
            Placeholder(Modifier.fillMaxSize())
            NetImage(
                images,
                detail.background ?: detail.poster,
                width * 2,
                Modifier.fillMaxSize(),
                alignment = Alignment.TopCenter,
            )
            Box(
                Modifier.fillMaxSize().background(
                    Brush.verticalGradient(
                        0f to Brand.Background.copy(alpha = 0.5f),
                        0.2f to Color.Transparent,
                        0.5f to Color.Transparent,
                        1f to Brand.Background,
                    ),
                ),
            )
            Column(
                Modifier.align(Alignment.BottomStart).padding(horizontal = 20.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (detail.logo != null) {
                    NetImage(
                        images,
                        detail.logo,
                        width,
                        Modifier.heightIn(max = 96.dp).widthIn(max = 280.dp),
                        contentDescription = detail.name,
                        contentScale = ContentScale.Fit,
                        alignment = Alignment.BottomStart,
                    )
                } else {
                    Text(
                        detail.name ?: detail.id,
                        style = MaterialTheme.typography.headlineLarge,
                        color = Brand.TextBright,
                    )
                }
                val facts = listOfNotNull(
                    detail.releaseInfo,
                    detail.runtime,
                    detail.imdbRating?.let { "★ $it" },
                ) + detail.genres.take(3)
                if (facts.isNotEmpty()) {
                    Text(facts.joinToString("  ·  "), style = MaterialTheme.typography.bodySmall, color = Brand.Text)
                }
            }
        }
        detail.description?.let {
            Text(
                it,
                style = MaterialTheme.typography.bodyMedium,
                color = Brand.Text,
                maxLines = if (expanded) Int.MAX_VALUE else 3,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .padding(horizontal = 20.dp)
                    .animateContentSize()
                    .clickable { expanded = !expanded },
            )
        }
    }
}

@Composable
private fun Actions(resume: LibraryItemView?, onResume: () -> Unit) {
    if (resume == null || !resume.resumable) return
    Column(Modifier.padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Button(onClick = onResume, modifier = Modifier.fillMaxWidth()) {
            CineoIcon(Icon.PLAYER_PLAY, filled = true, size = MaterialTheme.typography.titleMedium.fontSize)
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.resume))
        }
        Box(Modifier.fillMaxWidth().height(3.dp).clip(CircleShape).background(Brand.Surface)) {
            Box(Modifier.fillMaxWidth(resume.progress).fillMaxHeight().background(Brand.Accent))
        }
    }
}

@Composable
private fun Episodes(detail: DetailView, images: Images, onSelectVideo: (String) -> Unit) {
    val seasons = detail.videos.mapNotNull { it.season?.toInt() }.distinct().sortedWith(compareBy({ it == 0 }, { it }))
    val selectedSeason = detail.videos.firstOrNull { it.id == detail.selectedVideo }?.season?.toInt()
    var season by rememberSaveable(detail.id) { mutableStateOf(selectedSeason ?: seasons.firstOrNull()) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        RowHeader(stringResource(R.string.episodes), null)
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
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = Brand.Accent,
                            selectedLabelColor = Brand.OnAccent,
                        ),
                    )
                }
            }
        }
        detail.videos.filter { season == null || it.season?.toInt() == season }.forEach { video ->
            Episode(video, video.id == detail.selectedVideo, images) { onSelectVideo(video.id) }
        }
    }
}

@Composable
private fun Episode(video: VideoView, selected: Boolean, images: Images, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .background(if (selected) Brand.Surface else Color.Transparent)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(128.dp).aspectRatio(16f / 9f).clip(RoundedCornerShape(6.dp)).background(Brand.Surface)) {
            NetImage(images, video.thumbnail, 128.dp, Modifier.fillMaxSize())
            video.episode?.let {
                Text(
                    "$it",
                    style = MaterialTheme.typography.labelSmall,
                    color = Brand.TextBright,
                    modifier = Modifier
                        .align(Alignment.BottomStart)
                        .padding(4.dp)
                        .background(Color.Black.copy(alpha = 0.6f), RoundedCornerShape(4.dp))
                        .padding(horizontal = 6.dp, vertical = 2.dp),
                )
            }
        }
        Column(Modifier.weight(1f)) {
            Text(
                video.title,
                style = MaterialTheme.typography.bodyMedium,
                color = if (selected) Brand.Accent else Brand.TextBright,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            video.released?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Brand.TextDim) }
        }
    }
}

@Composable
private fun StreamCard(stream: StreamView, addonName: String, onPlay: () -> Unit) {
    val playable = stream.kind == SourceKindView.HTTP || stream.kind == SourceKindView.TORRENT
    val icon = if (stream.kind == SourceKindView.TORRENT) Icon.MAGNET else Icon.WORLD
    Row(
        Modifier
            .padding(horizontal = 16.dp)
            .fillMaxWidth()
            .clip(Rounded)
            .background(Brand.Panel)
            .border(1.dp, Brand.Rule, Rounded)
            .clickable(enabled = playable, onClick = onPlay)
            .alpha(if (playable) 1f else 0.5f)
            .padding(14.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(36.dp).clip(CircleShape).background(Brand.Surface), contentAlignment = Alignment.Center) {
            CineoIcon(icon, tint = if (playable) Brand.Accent else Brand.TextDim, size = MaterialTheme.typography.titleMedium.fontSize)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                (stream.name ?: addonName).replace('\n', ' '),
                style = MaterialTheme.typography.titleSmall,
                color = Brand.TextBright,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            val text = if (playable) stream.description else noticeText(NoticeView.UnsupportedSource(stream.kind))
            text?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = Brand.TextDim,
                    maxLines = 4,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (playable) {
            CineoIcon(
                Icon.PLAYER_PLAY,
                filled = true,
                tint = Brand.TextBright,
                modifier = Modifier.align(Alignment.CenterVertically),
            )
        }
    }
}
