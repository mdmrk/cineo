package io.github.mdmrk.cineo.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.material3.IconButton
import kotlinx.coroutines.delay
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.InstallView
import io.github.mdmrk.cineo.rust.ProblemView
import androidx.compose.material3.OutlinedButton
import androidx.compose.ui.platform.LocalUriHandler
import io.github.mdmrk.cineo.rust.PosterShape
import io.github.mdmrk.cineo.rust.PosterView
import io.github.mdmrk.cineo.rust.RowItems
import io.github.mdmrk.cineo.rust.RowView

val CardWidth = 108.dp
private val CardShape = RoundedCornerShape(6.dp)

@Composable
fun HomeScreen(
    view: AppView?,
    images: Images,
    onOpen: (PosterView) -> Unit,
    onOpenAddons: () -> Unit,
    library: LibraryActions,
    padding: PaddingValues,
) {
    val featured = view?.board.orEmpty().asSequence()
        .mapNotNull { (it.items as? RowItems.Ready)?.items }
        .flatten()
        .firstOrNull { it.background != null }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = padding.calculateBottomPadding() + 24.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        item(key = "featured") {
            if (featured != null) {
                Featured(featured, images, onOpen)
            } else {
                Spacer(Modifier.height(padding.calculateTopPadding() + 16.dp))
            }
        }
        val resume = view?.continueWatching.orEmpty()
        if (resume.isNotEmpty()) {
            item(key = "continue") { ContinueWatching(resume, images, library) }
        }
        when {
            view == null || (view.addons.isEmpty() && view.addonsLoading > 0u) -> items(3) { LoadingRow() }
            view.addons.isEmpty() -> item(key = "empty") {
                Column(
                    modifier = Modifier.padding(horizontal = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    PageTitle(stringResource(R.string.page_home))
                    Text(stringResource(R.string.home_empty), color = Brand.TextDim)
                    Button(onClick = onOpenAddons) { Text(stringResource(R.string.open_addons)) }
                }
            }
            else -> items(view.board, key = { it.key }) { row -> CatalogRow(row, images, onOpen) }
        }
    }
}

@Composable
private fun Featured(item: PosterView, images: Images, onOpen: (PosterView) -> Unit) {
    val width = LocalConfiguration.current.screenWidthDp.dp
    Box(Modifier.fillMaxWidth().aspectRatio(0.78f).clickable { onOpen(item) }) {
        Placeholder(Modifier.fillMaxSize())
        NetImage(images, item.background, width * 2, Modifier.fillMaxSize(), alignment = Alignment.TopCenter)
        Box(
            Modifier.fillMaxSize().background(
                Brush.verticalGradient(
                    0f to Brand.Background.copy(alpha = 0.6f),
                    0.25f to Color.Transparent,
                    0.55f to Color.Transparent,
                    1f to Brand.Background,
                ),
            ),
        )
        Column(
            Modifier.align(Alignment.BottomStart).padding(horizontal = 20.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (item.logo != null) {
                NetImage(
                    images,
                    item.logo,
                    width,
                    Modifier.heightIn(max = 88.dp).widthIn(max = 260.dp),
                    contentDescription = item.name,
                    contentScale = ContentScale.Fit,
                    alignment = Alignment.BottomStart,
                )
            } else {
                Text(item.name, style = MaterialTheme.typography.headlineLarge, color = Brand.TextBright)
            }
            Facts(item)
            Button(
                onClick = { onOpen(item) },
                colors = ButtonDefaults.buttonColors(containerColor = Brand.TextBright, contentColor = Brand.Background),
            ) {
                CineoIcon(Icon.INFO_CIRCLE, size = MaterialTheme.typography.titleMedium.fontSize)
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.details))
            }
        }
    }
}

@Composable
private fun Facts(item: PosterView) {
    val facts = listOfNotNull(item.releaseInfo, item.imdbRating?.let { "★ $it" }) + item.genres.take(2)
    if (facts.isNotEmpty()) {
        Text(facts.joinToString("  ·  "), style = MaterialTheme.typography.bodySmall, color = Brand.Text)
    }
}

@Composable
fun RowHeader(title: String, subtitle: String?) {
    Column(Modifier.padding(horizontal = 16.dp)) {
        Text(title, style = MaterialTheme.typography.titleMedium, color = Brand.TextBright)
        if (subtitle != null) {
            Text(subtitle, style = MaterialTheme.typography.labelSmall, color = Brand.TextDim)
        }
    }
}

@Composable
fun CatalogRow(row: RowView, images: Images, onOpen: (PosterView) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        RowHeader(catalogTitle(row), row.addonName)
        when (val items = row.items) {
            is RowItems.Loading -> PlaceholderStrip()
            is RowItems.Failed -> Row(
                Modifier.padding(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                CineoIcon(Icon.ALERT_CIRCLE, tint = Brand.Warning, size = MaterialTheme.typography.bodyMedium.fontSize)
                Text(problemText(items.problem), color = Brand.TextDim, style = MaterialTheme.typography.bodySmall)
            }
            is RowItems.Ready -> LazyRow(
                contentPadding = PaddingValues(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(items.items, key = { "${it.contentType}/${it.id}" }) { PosterCard(it, images, onOpen) }
            }
        }
    }
}

@Composable
private fun LoadingRow() {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Placeholder(Modifier.padding(horizontal = 16.dp).size(160.dp, 18.dp).clip(CardShape))
        PlaceholderStrip()
    }
}

@Composable
private fun PlaceholderStrip() {
    LazyRow(
        contentPadding = PaddingValues(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        userScrollEnabled = false,
    ) {
        items(6) { Placeholder(Modifier.width(CardWidth).aspectRatio(2f / 3f).clip(CardShape)) }
    }
}

@Composable
fun PosterCard(item: PosterView, images: Images, onOpen: (PosterView) -> Unit) {
    val ratio = when (item.shape) {
        PosterShape.POSTER -> 2f / 3f
        PosterShape.SQUARE -> 1f
        PosterShape.LANDSCAPE -> 16f / 9f
    }
    val width = if (ratio > 1f) CardWidth * 1.8f else CardWidth
    Column(Modifier.width(width).pressable { onOpen(item) }, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Artwork(item.name, item.poster, images, width, Modifier.aspectRatio(ratio))
        Text(
            item.name,
            style = MaterialTheme.typography.bodySmall,
            color = Brand.Text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
fun Artwork(name: String, url: String?, images: Images, width: Dp, modifier: Modifier = Modifier) {
    Box(
        modifier.fillMaxWidth().clip(CardShape).background(
            Brush.linearGradient(listOf(Brand.SurfaceHover, Brand.Panel)),
        ),
    ) {
        Column(
            Modifier.align(Alignment.Center).padding(10.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            CineoIcon(Icon.MOVIE, tint = Brand.TextFaint, size = MaterialTheme.typography.headlineSmall.fontSize)
            Text(
                name,
                style = MaterialTheme.typography.titleSmall,
                color = Brand.Text,
                textAlign = TextAlign.Center,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
        }
        NetImage(images, url, width, Modifier.fillMaxSize(), contentDescription = name)
    }
}

@Composable
fun InstallAddon(install: InstallView?, onInstall: (String) -> Unit) {
    var url by rememberSaveable { mutableStateOf("") }
    val busy = install is InstallView.Installing
    val submit = { if (!busy && url.isNotBlank()) onInstall(url.trim()) }
    Column(
        modifier = Modifier.padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(stringResource(R.string.addons_install), style = MaterialTheme.typography.titleMedium, color = Brand.TextBright)
        CineoField(
            value = url,
            onValueChange = { url = it },
            placeholder = "https://…/manifest.json",
            modifier = Modifier.fillMaxWidth(),
            leading = Icon.WORLD,
            trailing = {
                Button(
                    onClick = submit,
                    enabled = !busy && url.isNotBlank(),
                    modifier = Modifier.padding(end = 6.dp),
                ) { Text(stringResource(R.string.install)) }
            },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { submit() }),
        )
        when (install) {
            is InstallView.Installing -> Text(stringResource(R.string.installing))
            is InstallView.Installed -> Text(
                stringResource(R.string.installed_addon, install.name),
                color = MaterialTheme.colorScheme.primary,
            )
            is InstallView.Failed -> {
                Text(problemText(install.problem), color = MaterialTheme.colorScheme.error)
                val problem = install.problem
                if (problem is ProblemView.ConfigurationRequired) {
                    val uri = LocalUriHandler.current
                    OutlinedButton(onClick = { uri.openUri(problem.configureUrl) }) {
                        Text(stringResource(R.string.configure))
                    }
                }
            }
            null -> Unit
        }
    }
}

private const val NOTICE_MS = 8000L

@Composable
fun Notices(view: AppView?, aboveTabs: Boolean, onDismissNotice: () -> Unit) {
    val notice = view?.notice
    var expanded by remember(notice) { mutableStateOf(false) }
    LaunchedEffect(notice, expanded) {
        if (notice != null && !expanded) {
            delay(NOTICE_MS)
            onDismissNotice()
        }
    }
    Box(Modifier.fillMaxSize().safeDrawingPadding().padding(bottom = if (aboveTabs) 80.dp else 0.dp)) {
        AnimatedVisibility(
            visible = notice != null,
            enter = slideInVertically { it } + fadeIn(),
            exit = slideOutVertically { it } + fadeOut(),
            modifier = Modifier.align(Alignment.BottomCenter),
        ) {
            val (title, detail) = notice?.let { noticeParts(it) } ?: ("" to null)
            Row(
                Modifier
                    .padding(12.dp)
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .background(Brand.SurfaceHover)
                    .clickable(enabled = detail != null) { expanded = !expanded }
                    .padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                CineoIcon(Icon.ALERT_CIRCLE, tint = Brand.Warning, size = MaterialTheme.typography.titleMedium.fontSize)
                Column(Modifier.weight(1f).animateContentSize(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(title, style = MaterialTheme.typography.bodyMedium, color = Brand.TextBright)
                    if (detail != null) {
                        Text(
                            if (expanded) detail else stringResource(R.string.show_details),
                            style = MaterialTheme.typography.bodySmall,
                            color = if (expanded) Brand.TextDim else Brand.Accent,
                        )
                    }
                }
                IconButton(onClick = onDismissNotice) {
                    CineoIcon(Icon.X, tint = Brand.TextDim, size = MaterialTheme.typography.titleMedium.fontSize)
                }
            }
        }
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
