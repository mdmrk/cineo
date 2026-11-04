package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
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

private val CardWidth = 112.dp

@Composable
fun LibraryScreen(view: AppView?, images: Images, actions: LibraryActions, padding: PaddingValues) {
    val favorites = view?.favorites.orEmpty()
    val recent = view?.recent.orEmpty()
    LazyVerticalGrid(
        columns = GridCells.Adaptive(CardWidth),
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            start = 16.dp,
            end = 16.dp,
            top = padding.calculateTopPadding() + 16.dp,
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (favorites.isNotEmpty()) {
            item(key = "favorites", span = { GridItemSpan(maxLineSpan) }) {
                ShelfTitle(stringResource(R.string.favourites), favorites.size)
            }
            items(favorites, key = { "favorite/${it.id}" }) {
                LibraryCard(it, Shelf.FAVORITES, images, actions, Modifier.fillMaxWidth())
            }
        }
        item(key = "recent", span = { GridItemSpan(maxLineSpan) }) {
            ShelfTitle(stringResource(R.string.recently_played), recent.size)
        }
        if (recent.isEmpty()) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                Text(stringResource(R.string.library_empty))
            }
        }
        items(recent, key = { "recent/${it.id}" }) {
            LibraryCard(it, Shelf.LIBRARY, images, actions, Modifier.fillMaxWidth())
        }
    }
}

@Composable
private fun ShelfTitle(title: String, count: Int) {
    Column(Modifier.padding(top = 8.dp)) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        if (count > 0) {
            Text(stringResource(R.string.count_titles, count), style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
fun ContinueWatching(items: List<LibraryItemView>, images: Images, actions: LibraryActions) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            stringResource(R.string.continue_watching),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
        LazyRow(
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
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
    val widthPx = with(LocalDensity.current) { CardWidth.roundToPx() }
    var image by remember(item.poster) { mutableStateOf<ImageBitmap?>(null) }
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(item.poster) {
        image = item.poster?.let { images.load(it, widthPx) }
    }
    Box(modifier) {
        Card(
            Modifier.combinedClickable(
                onClick = { if (shelf == Shelf.FAVORITES) actions.open(item) else actions.resume(item) },
                onLongClick = { menu = true },
            ),
        ) {
            Box(Modifier.aspectRatio(2f / 3f)) {
                val loaded = image
                if (loaded != null) {
                    Image(
                        bitmap = loaded,
                        contentDescription = item.name,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Text(
                        item.name,
                        modifier = Modifier.align(Alignment.Center).padding(8.dp),
                        maxLines = 4,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (shelf != Shelf.FAVORITES && item.progress > 0f) {
                    LinearProgressIndicator(
                        progress = { item.progress },
                        modifier = Modifier.fillMaxWidth().align(Alignment.BottomCenter),
                    )
                }
            }
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
