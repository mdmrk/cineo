package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.background
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.LibraryItemView

enum class Shelf { CONTINUE_WATCHING, FAVORITES, LIBRARY }

class LibraryActions(
    val open: (LibraryItemView) -> Unit,
    val resume: (LibraryItemView) -> Unit,
    val setFavorite: (LibraryItemView, Boolean) -> Unit,
    val remove: (LibraryItemView) -> Unit,
)

@Composable
fun LibraryScreen(view: AppView?, images: Images, actions: LibraryActions, padding: PaddingValues) {
    val favorites = view?.favorites.orEmpty()
    val recent = view?.recent.orEmpty()
    var tab by rememberSaveable { mutableIntStateOf(if (recent.isEmpty() && favorites.isNotEmpty()) 1 else 0) }
    val shown = if (tab == 0) recent else favorites
    LazyVerticalGrid(
        columns = GridCells.Adaptive(CardWidth),
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            start = 16.dp,
            end = 16.dp,
            top = padding.calculateTopPadding() + 16.dp,
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        item(key = "title", span = { GridItemSpan(maxLineSpan) }) {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.page_library), style = MaterialTheme.typography.headlineMedium, color = Brand.TextBright)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(
                        stringResource(R.string.recently_played) to recent.size,
                        stringResource(R.string.favourites) to favorites.size,
                    ).forEachIndexed { index, (label, count) ->
                        FilterChip(
                            selected = tab == index,
                            onClick = { tab = index },
                            label = { Text(if (count > 0) "$label · $count" else label) },
                            colors = FilterChipDefaults.filterChipColors(
                                selectedContainerColor = Brand.Accent,
                                selectedLabelColor = Brand.OnAccent,
                            ),
                        )
                    }
                }
            }
        }
        if (shown.isEmpty()) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                Column(
                    Modifier.fillMaxWidth().padding(top = 64.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    CineoIcon(if (tab == 0) Icon.MOVIE else Icon.HEART, tint = Brand.TextFaint, size = 48.sp)
                    Text(stringResource(R.string.library_empty), color = Brand.TextDim)
                }
            }
        }
        items(shown, key = { "$tab/${it.id}" }) {
            LibraryCard(it, if (tab == 0) Shelf.LIBRARY else Shelf.FAVORITES, images, actions, Modifier.fillMaxWidth())
        }
    }
}

@Composable
fun ContinueWatching(items: List<LibraryItemView>, images: Images, actions: LibraryActions) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        RowHeader(stringResource(R.string.continue_watching), null)
        LazyRow(
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            items(items, key = { it.id }) {
                LibraryCard(it, Shelf.CONTINUE_WATCHING, images, actions, Modifier.width(CardWidth))
            }
        }
    }
}

@Composable
private fun LibraryCard(
    item: LibraryItemView,
    shelf: Shelf,
    images: Images,
    actions: LibraryActions,
    modifier: Modifier,
) {
    var menu by remember { mutableStateOf(false) }
    Box(modifier) {
        Column(
            Modifier.combinedClickable(
                onClick = { if (shelf == Shelf.FAVORITES) actions.open(item) else actions.resume(item) },
                onLongClick = { menu = true },
            ),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Box {
                Artwork(item.name, item.poster, images, CardWidth, Modifier.aspectRatio(2f / 3f))
                if (shelf != Shelf.FAVORITES && item.progress > 0f) {
                    Box(
                        Modifier.align(Alignment.BottomStart).fillMaxWidth().height(3.dp)
                            .background(Color.Black.copy(alpha = 0.6f)),
                    ) {
                        Box(Modifier.fillMaxWidth(item.progress).fillMaxHeight().background(Brand.Accent))
                    }
                }
                if (item.favorite && shelf != Shelf.FAVORITES) {
                    CineoIcon(
                        Icon.HEART,
                        filled = true,
                        tint = Brand.Accent,
                        size = 16.sp,
                        modifier = Modifier.align(Alignment.TopEnd).padding(6.dp),
                    )
                }
            }
            Text(
                item.name,
                style = MaterialTheme.typography.bodySmall,
                color = Brand.Text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(
                text = {
                    Text(stringResource(if (item.favorite) R.string.favourite_remove else R.string.favourite_add))
                },
                onClick = {
                    menu = false
                    actions.setFavorite(item, !item.favorite)
                },
            )
            val remove = when (shelf) {
                Shelf.CONTINUE_WATCHING -> R.string.remove_continue
                Shelf.LIBRARY -> if (item.played) R.string.remove_library else null
                Shelf.FAVORITES -> null
            }
            if (remove != null) {
                DropdownMenuItem(
                    text = { Text(stringResource(remove)) },
                    onClick = {
                        menu = false
                        actions.remove(item)
                    },
                )
            }
        }
    }
}
