package io.github.mdmrk.cineo.ui

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
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
                        Chip(if (count > 0) "$label · $count" else label, tab == index) { tab = index }
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
    var sheet by remember { mutableStateOf(false) }
    Column(
        modifier.combinedClickable(
            onClick = { if (shelf == Shelf.FAVORITES) actions.open(item) else actions.resume(item) },
            onLongClick = { sheet = true },
        ),
        verticalArrangement = Arrangement.spacedBy(2.dp),
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
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                item.name,
                style = MaterialTheme.typography.bodySmall,
                color = Brand.Text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Box(
                Modifier.size(32.dp).clip(CircleShape).clickable { sheet = true },
                contentAlignment = Alignment.Center,
            ) {
                CineoIcon(Icon.DOTS_VERTICAL, tint = Brand.TextDim, size = 18.sp, label = stringResource(R.string.more_actions))
            }
        }
    }
    if (sheet) ItemSheet(item, shelf, images, actions) { sheet = false }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ItemSheet(
    item: LibraryItemView,
    shelf: Shelf,
    images: Images,
    actions: LibraryActions,
    onClose: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onClose, containerColor = Brand.Panel) {
        Column(Modifier.fillMaxWidth().padding(bottom = 16.dp)) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Artwork(item.name, item.poster, images, 44.dp, Modifier.width(44.dp).aspectRatio(2f / 3f))
                Text(
                    item.name,
                    style = MaterialTheme.typography.titleMedium,
                    color = Brand.TextBright,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            HorizontalDivider(color = Brand.Rule)
            if (shelf != Shelf.FAVORITES) {
                SheetAction(Icon.INFO_CIRCLE, stringResource(R.string.details)) {
                    onClose()
                    actions.open(item)
                }
            }
            SheetAction(
                Icon.HEART,
                stringResource(if (item.favorite) R.string.favourite_remove else R.string.favourite_add),
                filled = item.favorite,
                tint = if (item.favorite) Brand.Accent else Brand.Text,
            ) {
                onClose()
                actions.setFavorite(item, !item.favorite)
            }
            val remove = when (shelf) {
                Shelf.CONTINUE_WATCHING -> Icon.X to R.string.remove_continue
                Shelf.LIBRARY -> if (item.played) Icon.TRASH to R.string.remove_library else null
                Shelf.FAVORITES -> null
            }
            if (remove != null) {
                SheetAction(remove.first, stringResource(remove.second), tint = Brand.Danger) {
                    onClose()
                    actions.remove(item)
                }
            }
        }
    }
}

@Composable
private fun SheetAction(
    icon: Icon,
    label: String,
    filled: Boolean = false,
    tint: Color = Brand.Text,
    onClick: () -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        CineoIcon(icon, filled = filled, tint = tint, size = 22.sp, modifier = Modifier.width(24.dp))
        Text(label, style = MaterialTheme.typography.bodyLarge, color = if (tint == Brand.Danger) tint else Brand.Text)
    }
}
