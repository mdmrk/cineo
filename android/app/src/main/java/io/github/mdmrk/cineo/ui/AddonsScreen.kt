package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.AddonView
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.Cineo

@Composable
fun AddonsScreen(view: AppView?, cineo: Cineo, padding: PaddingValues) {
    val addons = view?.addons.orEmpty()
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
            Text(
                stringResource(R.string.addons_installed),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp),
            )
            Text(
                stringResource(if (addons.isEmpty()) R.string.addons_none else R.string.addons_dek),
                style = MaterialTheme.typography.bodySmall,
                color = Brand.TextDim,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
        itemsIndexed(addons, key = { _, addon -> addon.transport }) { index, addon ->
            AddonRow(
                addon,
                canMoveUp = index > 0,
                canMoveDown = index < addons.size - 1,
                onMove = { by -> cineo.moveAddon(index.toUInt(), (index + by).toUInt()) },
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
private fun AddonRow(
    addon: AddonView,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onMove: (Int) -> Unit,
    onRemove: () -> Unit,
) {
    ListItem(
        colors = ListItemDefaults.colors(containerColor = Brand.Background),
        leadingContent = { CineoIcon(Icon.PUZZLE, tint = Brand.TextDim) },
        headlineContent = { Text(addon.name, color = Brand.TextBright) },
        supportingContent = { Text("v${addon.version}", color = Brand.TextDim) },
        trailingContent = {
            Row {
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
