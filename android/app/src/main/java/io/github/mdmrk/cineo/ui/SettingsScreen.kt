package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.AddonView
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.SettingChange
import io.github.mdmrk.cineo.rust.SettingsView

@Composable
fun SettingsScreen(settings: SettingsView, addons: List<AddonView>, cineo: Cineo, padding: PaddingValues) {
    val change = cineo::changeSetting
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            top = padding.calculateTopPadding() + 16.dp,
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
    ) {
        item(key = "player") {
            Section(stringResource(R.string.section_player))
            Toggle(
                stringResource(R.string.hardware_decoding),
                stringResource(R.string.hardware_decoding_help),
                settings.hardwareDecoding,
            ) { change(SettingChange.HardwareDecoding(it)) }
            Choices(
                stringResource(R.string.seek_step),
                stringResource(R.string.seek_step_help),
                settings.seekStepsS,
                settings.seekStepS,
                { stringResource(R.string.seconds, it.toInt()) },
            ) { change(SettingChange.SeekStep(it)) }
            Choices(
                stringResource(R.string.watched_at),
                stringResource(R.string.watched_at_help),
                settings.watchedAtChoices,
                settings.watchedAtPercent,
                { stringResource(R.string.watched_percent, it.toInt()) },
            ) { change(SettingChange.WatchedAt(it)) }
        }
        item(key = "languages") {
            Section(stringResource(R.string.section_languages))
            LanguagePicker(
                stringResource(R.string.audio_language),
                stringResource(R.string.audio_language_help),
                settings.languages,
                settings.audioLanguage,
            ) { change(SettingChange.AudioLanguage(it)) }
            LanguagePicker(
                stringResource(R.string.subtitle_language),
                stringResource(R.string.subtitle_language_help),
                settings.languages,
                settings.subtitleLanguage,
            ) { change(SettingChange.SubtitleLanguage(it)) }
        }
        item(key = "addons") {
            Section(stringResource(R.string.page_addons))
            Text(
                stringResource(if (addons.isEmpty()) R.string.addons_none else R.string.addons_dek),
                style = MaterialTheme.typography.bodySmall,
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
        item(key = "data") {
            Section(stringResource(R.string.section_data))
            Toggle(
                stringResource(R.string.private_network),
                stringResource(R.string.private_network_help),
                settings.allowPrivateNetwork,
            ) { change(SettingChange.AllowPrivateNetwork(it)) }
            Confirmed(
                stringResource(R.string.clear_history),
                stringResource(R.string.clear_history_help),
                stringResource(R.string.clear_confirm_title),
                stringResource(R.string.clear_confirm_text),
                stringResource(R.string.clear),
                cineo::clearLibrary,
            )
            Confirmed(
                stringResource(R.string.reset_settings),
                stringResource(R.string.reset_settings_help),
                stringResource(R.string.reset_confirm_title),
                stringResource(R.string.reset_confirm_text),
                stringResource(R.string.reset),
                cineo::resetSettings,
            )
        }
    }
}

@Composable
private fun Section(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 24.dp, bottom = 4.dp),
    )
}

@Composable
private fun Toggle(title: String, help: String, on: Boolean, onChange: (Boolean) -> Unit) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = { Text(help) },
        trailingContent = { Switch(checked = on, onCheckedChange = onChange) },
        modifier = Modifier.clickable { onChange(!on) },
    )
}

@Composable
private fun Choices(
    title: String,
    help: String,
    choices: List<UInt>,
    current: UInt,
    label: @Composable (UInt) -> String,
    onPick: (UInt) -> Unit,
) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = {
            Column {
                Text(help)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    choices.forEach { choice ->
                        FilterChip(
                            selected = choice == current,
                            onClick = { onPick(choice) },
                            label = { Text(label(choice)) },
                        )
                    }
                }
            }
        },
    )
}

@Composable
private fun LanguagePicker(
    title: String,
    help: String,
    languages: List<String>,
    current: String?,
    onPick: (String?) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val none = stringResource(R.string.language_none)
    val names = languages.map { it to (languageName(it)?.let { id -> stringResource(id) } ?: it) }
        .sortedBy { it.second }
    val currentName = names.firstOrNull { it.first == current }?.second ?: none
    Box {
        ListItem(
            headlineContent = { Text(title) },
            supportingContent = { Text(help) },
            trailingContent = { Text(currentName, color = MaterialTheme.colorScheme.primary) },
            modifier = Modifier.clickable { open = true },
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(text = { Text(none) }, onClick = { open = false; onPick(null) })
            names.forEach { (code, name) ->
                DropdownMenuItem(text = { Text(name) }, onClick = { open = false; onPick(code) })
            }
        }
    }
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
        headlineContent = { Text(addon.name) },
        supportingContent = {
            Column {
                Text("v${addon.version}", style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(onClick = { onMove(-1) }, enabled = canMoveUp) {
                        Text(stringResource(R.string.move_up))
                    }
                    TextButton(onClick = { onMove(1) }, enabled = canMoveDown) {
                        Text(stringResource(R.string.move_down))
                    }
                    TextButton(onClick = onRemove) { Text(stringResource(R.string.remove)) }
                }
            }
        },
    )
}

@Composable
private fun Confirmed(
    title: String,
    help: String,
    confirmTitle: String,
    confirmText: String,
    confirmLabel: String,
    onConfirm: () -> Unit,
) {
    var asking by remember { mutableStateOf(false) }
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = { Text(help) },
        trailingContent = { OutlinedButton(onClick = { asking = true }) { Text(confirmLabel) } },
    )
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            title = { Text(confirmTitle) },
            text = { Text(confirmText) },
            confirmButton = {
                TextButton(onClick = { asking = false; onConfirm() }) { Text(confirmLabel) }
            },
            dismissButton = {
                TextButton(onClick = { asking = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}
