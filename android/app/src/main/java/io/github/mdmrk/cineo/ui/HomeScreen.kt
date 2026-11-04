package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
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
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.InstallView
import io.github.mdmrk.cineo.rust.PosterShapeView
import io.github.mdmrk.cineo.rust.PosterView
import io.github.mdmrk.cineo.rust.RowItems
import io.github.mdmrk.cineo.rust.RowView

private val PosterWidth = 112.dp

@Composable
fun HomeScreen(
    view: AppView?,
    images: Images,
    onInstall: (String) -> Unit,
    onOpen: (PosterView) -> Unit,
    library: LibraryActions,
    padding: PaddingValues,
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            top = padding.calculateTopPadding() + 16.dp,
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(24.dp),
    ) {
        val resume = view?.continueWatching.orEmpty()
        if (resume.isNotEmpty()) {
            item(key = "continue") { ContinueWatching(resume, images, library) }
        }
        item(key = "install") { InstallAddon(view?.install, onInstall) }
        when {
            view == null || (view.addons.isEmpty() && view.addonsLoading > 0u) -> item(key = "loading") {
                Text(
                    stringResource(R.string.loading_addons),
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
            view.addons.isEmpty() -> item(key = "empty") {
                Text(
                    stringResource(R.string.home_empty),
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
            else -> items(view.board, key = { it.key }) { row -> CatalogRow(row, images, onOpen) }
        }
    }
}

@Composable
private fun InstallAddon(install: InstallView?, onInstall: (String) -> Unit) {
    var url by rememberSaveable { mutableStateOf("") }
    val busy = install is InstallView.Installing
    val submit = { if (!busy && url.isNotBlank()) onInstall(url.trim()) }
    Column(
        modifier = Modifier.padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(stringResource(R.string.addons_install), style = MaterialTheme.typography.titleMedium)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = url,
                onValueChange = { url = it },
                modifier = Modifier.weight(1f),
                singleLine = true,
                placeholder = { Text("https://…/manifest.json") },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { submit() }),
            )
            Button(onClick = submit, enabled = !busy) { Text(stringResource(R.string.install)) }
        }
        when (install) {
            is InstallView.Installing -> Text(stringResource(R.string.installing))
            is InstallView.Installed -> Text(
                stringResource(R.string.installed_addon, install.name),
                color = MaterialTheme.colorScheme.primary,
            )
            is InstallView.Failed -> Text(problemText(install.problem), color = MaterialTheme.colorScheme.error)
            null -> Unit
        }
    }
}

@Composable
private fun CatalogRow(row: RowView, images: Images, onOpen: (PosterView) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            targetLabel(row),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
        when (val items = row.items) {
            is RowItems.Loading -> Box(Modifier.fillMaxWidth().padding(16.dp)) {
                CircularProgressIndicator(Modifier.align(Alignment.Center))
            }
            is RowItems.Failed -> Text(
                problemText(items.problem),
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            is RowItems.Ready -> LazyRow(
                contentPadding = PaddingValues(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(items.items, key = { "${it.contentType}/${it.id}" }) { Poster(it, images, onOpen) }
            }
        }
    }
}

@Composable
private fun Poster(item: PosterView, images: Images, onOpen: (PosterView) -> Unit) {
    val widthPx = with(LocalDensity.current) { PosterWidth.roundToPx() }
    var image by remember(item.poster) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(item.poster) {
        image = item.poster?.let { images.load(it, widthPx) }
    }
    val ratio = when (item.shape) {
        PosterShapeView.POSTER -> 2f / 3f
        PosterShapeView.SQUARE -> 1f
        PosterShapeView.LANDSCAPE -> 16f / 9f
    }
    Card(
        onClick = { onOpen(item) },
        modifier = Modifier.width(if (ratio > 1f) PosterWidth * 1.8f else PosterWidth),
    ) {
        Box(Modifier.aspectRatio(ratio)) {
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
        }
    }
}

@Composable
fun Notices(view: AppView?, onDismissNotice: () -> Unit) {
    val snackbar = remember { SnackbarHostState() }
    val notice = view?.notice?.let { noticeText(it) }
    val dismiss = stringResource(R.string.dismiss)
    LaunchedEffect(notice) {
        if (notice != null) {
            val result = snackbar.showSnackbar(notice, dismiss, duration = SnackbarDuration.Long)
            if (result == SnackbarResult.ActionPerformed || result == SnackbarResult.Dismissed) {
                onDismissNotice()
            }
        }
    }
    Box(Modifier.fillMaxSize().safeDrawingPadding()) {
        SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter))
    }
}

@Composable
fun StartFailed(reason: String) {
    Scaffold { padding ->
        Text(
            stringResource(R.string.start_failed, reason),
            modifier = Modifier.padding(padding).padding(16.dp),
            color = MaterialTheme.colorScheme.error,
        )
    }
}
