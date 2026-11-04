package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.AddonListItems
import io.github.mdmrk.cineo.rust.AddonView
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.InstallView
import io.github.mdmrk.cineo.rust.ListedAddonView

private val LogoShape = RoundedCornerShape(8.dp)

@Composable
fun AddonsScreen(view: AppView?, images: Images, cineo: Cineo, padding: PaddingValues) {
    val addons = view?.addons.orEmpty()
    val lists = view?.addonLists.orEmpty()
    val uri = LocalUriHandler.current
    var selected by rememberSaveable { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) { cineo.loadAddonLists() }
    val list = lists.firstOrNull { it.key == selected } ?: lists.firstOrNull()
    val busy = view?.install is InstallView.Installing
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            top = padding.calculateTopPadding() + 16.dp,
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item(key = "title") { PageTitle(stringResource(R.string.page_addons)) }
        item(key = "install") { InstallAddon(view?.install, cineo::installAddon) }
        item(key = "installed") {
            Section(
                stringResource(R.string.addons_installed),
                stringResource(if (addons.isEmpty()) R.string.addons_none else R.string.addons_dek),
            )
        }
        itemsIndexed(addons, key = { _, addon -> addon.transport }) { index, addon ->
            AddonRow(
                addon,
                images,
                canMoveUp = index > 0,
                canMoveDown = index < addons.size - 1,
                onMove = { by -> cineo.moveAddon(index.toUInt(), (index + by).toUInt()) },
                onConfigure = addon.configureUrl?.let { url -> { uri.openUri(url) } },
                onRemove = { cineo.removeAddon(addon.transport) },
            )
        }
        items(view?.unavailableAddons.orEmpty(), key = { it }) { transport ->
            ListItem(
                colors = ListItemDefaults.colors(containerColor = Brand.Background),
                leadingContent = { CineoIcon(Icon.ALERT_CIRCLE, tint = Brand.Warning) },
                headlineContent = { Text(transport, color = Brand.TextBright, maxLines = 1) },
                supportingContent = { Text(stringResource(R.string.badge_unavailable), color = Brand.Warning) },
                trailingContent = {
                    IconButton(onClick = { cineo.removeAddon(transport) }) {
                        CineoIcon(Icon.TRASH, tint = Brand.Danger)
                    }
                },
            )
        }
        if (lists.isNotEmpty() && list != null) {
            item(key = "discover") {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Section(stringResource(R.string.addons_discover), stringResource(R.string.addons_discover_note))
                    val severalAddons = lists.any { it.addonName != lists.first().addonName }
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = 16.dp),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        items(lists, key = { it.key }) { entry ->
                            FilterChip(
                                selected = entry.key == list.key,
                                onClick = { selected = entry.key },
                                label = { Text(if (severalAddons) "${entry.name} · ${entry.addonName}" else entry.name) },
                                colors = FilterChipDefaults.filterChipColors(
                                    selectedContainerColor = Brand.Accent,
                                    selectedLabelColor = Brand.OnAccent,
                                ),
                            )
                        }
                    }
                }
            }
            when (val items = list.items) {
                is AddonListItems.Loading -> items(4, key = { "loading-$it" }) {
                    Placeholder(Modifier.padding(horizontal = 16.dp).fillMaxWidth().height(72.dp).clip(LogoShape))
                }
                is AddonListItems.Failed -> item(key = "failed") {
                    Row(
                        Modifier.padding(horizontal = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(problemText(items.problem), color = Brand.TextDim, modifier = Modifier.weight(1f))
                        TextButton(onClick = cineo::loadAddonLists) { Text(stringResource(R.string.retry)) }
                    }
                }
                is AddonListItems.Ready -> {
                    if (items.addons.isEmpty()) {
                        item(key = "empty") {
                            Text(
                                stringResource(R.string.addons_list_empty),
                                color = Brand.TextDim,
                                modifier = Modifier.padding(horizontal = 16.dp),
                            )
                        }
                    }
                    items(items.addons, key = { "listed/${it.transport}" }) { addon ->
                        ListedAddon(
                            addon,
                            images,
                            busy,
                            onInstall = { cineo.installAddon(addon.transport) },
                            onConfigure = { uri.openUri(addon.configureUrl) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun PageTitle(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.headlineMedium,
        color = Brand.TextBright,
        modifier = Modifier.padding(horizontal = 16.dp),
    )
}

@Composable
private fun Section(title: String, note: String) {
    Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 20.dp)) {
        Text(title, style = MaterialTheme.typography.titleMedium, color = Brand.TextBright)
        Text(note, style = MaterialTheme.typography.bodySmall, color = Brand.TextDim)
    }
}

@Composable
private fun AddonLogo(name: String, logo: String?, images: Images) {
    Box(
        Modifier.size(44.dp).clip(LogoShape).background(Brand.Surface),
        contentAlignment = Alignment.Center,
    ) {
        if (logo == null) {
            Text(
                name.take(1).uppercase(),
                style = MaterialTheme.typography.titleMedium,
                color = Brand.TextDim,
            )
        }
        NetImage(images, logo, 44.dp, Modifier.fillMaxSize(), contentDescription = name, contentScale = ContentScale.Fit)
    }
}

@Composable
private fun AddonRow(
    addon: AddonView,
    images: Images,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onMove: (Int) -> Unit,
    onConfigure: (() -> Unit)?,
    onRemove: () -> Unit,
) {
    ListItem(
        colors = ListItemDefaults.colors(containerColor = Brand.Background),
        leadingContent = { AddonLogo(addon.name, addon.logo, images) },
        headlineContent = { Text(addon.name, color = Brand.TextBright, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = { Text("v${addon.version}", color = Brand.TextDim) },
        trailingContent = {
            Row {
                if (onConfigure != null) {
                    IconButton(onClick = onConfigure) { CineoIcon(Icon.SETTINGS, tint = Brand.Text) }
                }
                IconButton(onClick = { onMove(-1) }, enabled = canMoveUp) {
                    CineoIcon(Icon.ARROW_UP, tint = if (canMoveUp) Brand.Text else Brand.TextFaint)
                }
                IconButton(onClick = { onMove(1) }, enabled = canMoveDown) {
                    CineoIcon(Icon.ARROW_DOWN, tint = if (canMoveDown) Brand.Text else Brand.TextFaint)
                }
                IconButton(onClick = onRemove) {
                    CineoIcon(Icon.TRASH, tint = Brand.Danger)
                }
            }
        },
    )
}

@Composable
private fun ListedAddon(
    addon: ListedAddonView,
    images: Images,
    busy: Boolean,
    onInstall: () -> Unit,
    onConfigure: () -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        AddonLogo(addon.name, addon.logo, images)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                addon.name,
                style = MaterialTheme.typography.titleSmall,
                color = Brand.TextBright,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            addon.description?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = Brand.TextDim,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (addon.needsSetup && !addon.installed) {
                Text(stringResource(R.string.badge_configure), style = MaterialTheme.typography.labelSmall, color = Brand.Warning)
            }
        }
        when {
            addon.installed -> Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                CineoIcon(Icon.CHECK, tint = Brand.Accent, size = 18.sp)
                Text(stringResource(R.string.addon_installed), style = MaterialTheme.typography.labelLarge, color = Brand.Accent)
            }
            addon.needsSetup -> OutlinedButton(onClick = onConfigure) { Text(stringResource(R.string.configure)) }
            else -> Button(onClick = onInstall, enabled = !busy) { Text(stringResource(R.string.install)) }
        }
    }
}
