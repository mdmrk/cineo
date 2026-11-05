package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.window.Dialog
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.ChoiceView
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.RangeView
import io.github.mdmrk.cineo.rust.SettingChange
import io.github.mdmrk.cineo.rust.SettingsView
import io.github.mdmrk.cineo.rust.ToggleView
import io.github.mdmrk.cineo.rust.version
import java.text.Collator
import kotlin.math.roundToInt

private val GroupShape = RoundedCornerShape(10.dp)

@Composable
fun SettingsScreen(settings: SettingsView, cineo: Cineo, padding: PaddingValues) {
    val change = cineo::changeSetting
    val locale = LocalConfiguration.current.locales[0]
    val collator = remember(locale) { Collator.getInstance(locale) }
    val none = stringResource(R.string.language_none)
    val languageNames = settings.languages.associateWith { languageName(it)?.let { id -> stringResource(id) } ?: it }
    val languages = listOf<String?>(null) + settings.languages.sortedWith(compareBy(collator) { languageNames.getValue(it) })
    val languageLabel = @Composable { code: String? -> code?.let { languageNames[it] ?: it } ?: none }
    val set = { key: String, value: String -> change(SettingChange.Value(key, value)) }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            start = 16.dp,
            end = 16.dp,
            top = padding.calculateTopPadding() + 16.dp,
            bottom = padding.calculateBottomPadding() + 24.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item(key = "title") {
            Text(stringResource(R.string.page_settings), style = MaterialTheme.typography.headlineMedium, color = Brand.TextBright)
        }
        item(key = "player") {
            Group(stringResource(R.string.section_player)) {
                Toggle(
                    stringResource(R.string.hardware_decoding),
                    stringResource(R.string.hardware_decoding_help),
                    settings.hardwareDecoding,
                ) { change(SettingChange.HardwareDecoding(it)) }
                Divider()
                Picker(
                    stringResource(R.string.seek_step),
                    stringResource(R.string.seek_step_help),
                    settings.seekStepsS,
                    settings.seekStepS,
                    { stringResource(R.string.seconds, it.toInt()) },
                ) { change(SettingChange.SeekStep(it)) }
                Divider()
                Picker(
                    stringResource(R.string.watched_at),
                    stringResource(R.string.watched_at_help),
                    settings.watchedAtChoices,
                    settings.watchedAtPercent,
                    { stringResource(R.string.watched_percent, it.toInt()) },
                ) { change(SettingChange.WatchedAt(it)) }
                Divider()
                Choice(
                    stringResource(R.string.hide_controls),
                    stringResource(R.string.hide_controls_help),
                    settings.hideControls,
                    { stringResource(R.string.seconds_decimal, it) },
                    set,
                )
            }
        }
        item(key = "languages") {
            Group(stringResource(R.string.section_languages)) {
                Picker(
                    stringResource(R.string.audio_language),
                    stringResource(R.string.audio_language_help),
                    languages,
                    settings.audioLanguage,
                    languageLabel,
                ) { change(SettingChange.AudioLanguage(it)) }
                Divider()
                Picker(
                    stringResource(R.string.audio_language_second),
                    stringResource(R.string.audio_language_second_help),
                    languages,
                    settings.secondaryAudioLanguage,
                    languageLabel,
                ) { change(SettingChange.SecondaryAudioLanguage(it)) }
                Divider()
                Picker(
                    stringResource(R.string.subtitle_language),
                    stringResource(R.string.subtitle_language_help),
                    languages,
                    settings.subtitleLanguage,
                    languageLabel,
                ) { change(SettingChange.SubtitleLanguage(it)) }
                Divider()
                Picker(
                    stringResource(R.string.subtitle_language_second),
                    stringResource(R.string.subtitle_language_second_help),
                    languages,
                    settings.secondarySubtitleLanguage,
                    languageLabel,
                ) { change(SettingChange.SecondarySubtitleLanguage(it)) }
            }
        }
        item(key = "subtitles") {
            val style = stringResource(R.string.subtitle_style_help)
            Group(stringResource(R.string.section_subtitles)) {
                Range(stringResource(R.string.subtitle_size), style, settings.subtitleSize, 5, set)
                Divider()
                Choice(stringResource(R.string.subtitle_font), style, settings.subtitleFont, { fontName(it) }, set)
                Divider()
                Switch(stringResource(R.string.subtitle_bold), style, settings.subtitleBold, set)
                Divider()
                Choice(stringResource(R.string.subtitle_color), style, settings.subtitleColor, { colorName(it) }, set)
                Divider()
                Range(stringResource(R.string.subtitle_opacity), style, settings.subtitleOpacity, 5, set)
                Divider()
                Choice(
                    stringResource(R.string.subtitle_outline),
                    stringResource(R.string.subtitle_outline_help),
                    settings.subtitleOutline,
                    { outlineName(it) },
                    set,
                )
                Divider()
                Choice(
                    stringResource(R.string.subtitle_background),
                    style,
                    settings.subtitleBackground,
                    { backgroundName(it) },
                    set,
                )
                Divider()
                Range(
                    stringResource(R.string.subtitle_raise),
                    stringResource(R.string.subtitle_raise_help),
                    settings.subtitlePosition,
                    1,
                    set,
                )
                Divider()
                Switch(
                    stringResource(R.string.subtitle_keep_styles),
                    stringResource(R.string.subtitle_keep_styles_help),
                    settings.keepSubtitleStyles,
                    set,
                )
            }
        }
        item(key = "torrents") {
            val next = stringResource(R.string.next_torrent)
            Group(stringResource(R.string.section_torrents)) {
                Toggle(
                    stringResource(R.string.p2p_enabled),
                    stringResource(R.string.p2p_help),
                    settings.p2pEnabled,
                ) { change(SettingChange.P2pEnabled(it)) }
                Divider()
                Switch(
                    stringResource(R.string.torrent_upload),
                    stringResource(R.string.torrent_upload_help),
                    settings.torrentUpload,
                    set,
                )
                Divider()
                Choice(
                    stringResource(R.string.torrent_storage),
                    stringResource(R.string.torrent_storage_help),
                    settings.torrentStorage,
                    { storageName(it) },
                    set,
                )
                Divider()
                Choice(stringResource(R.string.download_limit), next, settings.downloadLimit, { rateName(it) }, set)
                Divider()
                Choice(stringResource(R.string.upload_limit), next, settings.uploadLimit, { rateName(it) }, set)
                Divider()
                Choice(
                    stringResource(R.string.peer_limit),
                    stringResource(R.string.peer_limit_help),
                    settings.peerLimit,
                    { it },
                    set,
                )
                Divider()
                Switch(stringResource(R.string.dht), stringResource(R.string.dht_help), settings.torrentDht, set)
                Divider()
                Port(stringResource(R.string.torrent_port), stringResource(R.string.torrent_port_help), settings.torrentPort, set)
            }
        }
        item(key = "data") {
            Group(stringResource(R.string.section_data)) {
                Toggle(
                    stringResource(R.string.private_network),
                    stringResource(R.string.private_network_help),
                    settings.allowPrivateNetwork,
                ) { change(SettingChange.AllowPrivateNetwork(it)) }
                Divider()
                Confirmed(
                    stringResource(R.string.clear_history),
                    stringResource(R.string.clear_history_help),
                    stringResource(R.string.clear_confirm_title),
                    stringResource(R.string.clear_confirm_text),
                    stringResource(R.string.clear),
                    cineo::clearLibrary,
                )
                Divider()
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
        item(key = "about") {
            Group(stringResource(R.string.section_about)) {
                ListItem(
                    colors = rowColors,
                    headlineContent = { Text(stringResource(R.string.app_version, version())) },
                )
                Divider()
                Licenses()
            }
        }
    }
}

private const val LICENSES = "licenses"

@Composable
private fun Licenses() {
    val assets = LocalContext.current.assets
    var listing by remember { mutableStateOf(false) }
    var shown by remember { mutableStateOf<String?>(null) }
    ListItem(
        colors = rowColors,
        headlineContent = { Text(stringResource(R.string.licenses)) },
        supportingContent = { Text(stringResource(R.string.licenses_help), style = MaterialTheme.typography.bodySmall) },
        trailingContent = { CineoIcon(Icon.CHEVRON_RIGHT, tint = Brand.TextDim, size = 20.sp) },
        modifier = Modifier.clickable { listing = true },
    )
    if (listing) {
        val files = remember { assets.list(LICENSES).orEmpty().sorted() }
        PickerDialog(stringResource(R.string.licenses), files.map { it to it }, null, onDismiss = { listing = false }) {
            shown = it
        }
    }
    shown?.let { name ->
        val text = remember(name) { assets.open("$LICENSES/$name").bufferedReader().use { it.readText() } }
        Dialog(onDismissRequest = { shown = null }) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .heightIn(max = 640.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(Brand.Panel)
                    .padding(vertical = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    name,
                    style = MaterialTheme.typography.titleLarge,
                    color = Brand.TextBright,
                    modifier = Modifier.padding(horizontal = 20.dp),
                )
                Text(
                    text,
                    style = MaterialTheme.typography.bodySmall,
                    fontFamily = FontFamily.Monospace,
                    color = Brand.TextDim,
                    modifier = Modifier
                        .weight(1f, fill = false)
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 20.dp),
                )
                TextButton(onClick = { shown = null }, modifier = Modifier.align(Alignment.End).padding(end = 8.dp)) {
                    Text(stringResource(R.string.close))
                }
            }
        }
    }
}

@Composable
private fun Group(title: String, content: @Composable () -> Unit) {
    Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = MaterialTheme.typography.labelLarge, color = Brand.Accent)
        Column(Modifier.fillMaxWidth().clip(GroupShape).background(Brand.Panel)) { content() }
    }
}

@Composable
private fun Divider() {
    HorizontalDivider(color = Brand.Rule, modifier = Modifier.padding(horizontal = 16.dp))
}

private val rowColors @Composable get() = ListItemDefaults.colors(
    containerColor = Brand.Panel,
    headlineColor = Brand.TextBright,
    supportingColor = Brand.TextDim,
)

@Composable
private fun Toggle(title: String, help: String, on: Boolean, onChange: (Boolean) -> Unit) {
    ListItem(
        colors = rowColors,
        headlineContent = { Text(title) },
        supportingContent = { Text(help, style = MaterialTheme.typography.bodySmall) },
        trailingContent = {
            Switch(
                checked = on,
                onCheckedChange = onChange,
                colors = SwitchDefaults.colors(checkedTrackColor = Brand.Accent, checkedThumbColor = Brand.OnAccent),
            )
        },
        modifier = Modifier.clickable { onChange(!on) },
    )
}

@Composable
private fun Switch(title: String, help: String, view: ToggleView, set: (String, String) -> Unit) {
    Toggle(title, help, view.on) { set(view.key, it.toString()) }
}

@Composable
private fun Choice(
    title: String,
    help: String,
    view: ChoiceView,
    label: @Composable (String) -> String,
    set: (String, String) -> Unit,
) {
    Picker(title, help, view.options, view.value, label) { set(view.key, it) }
}

@Composable
private fun Range(title: String, help: String, view: RangeView, step: Int, set: (String, String) -> Unit) {
    var dragged by remember(view.value) { mutableFloatStateOf(view.value.toFloat()) }
    val min = view.min.toInt()
    val max = view.max.toInt()
    ListItem(
        colors = rowColors,
        headlineContent = { Text(title) },
        supportingContent = {
            Column {
                Text(help, style = MaterialTheme.typography.bodySmall)
                Slider(
                    value = dragged,
                    onValueChange = { dragged = (min + ((it - min) / step).roundToInt() * step).toFloat() },
                    onValueChangeFinished = { set(view.key, dragged.toInt().toString()) },
                    valueRange = min.toFloat()..max.toFloat(),
                    colors = SliderDefaults.colors(thumbColor = Brand.Accent, activeTrackColor = Brand.Accent),
                )
            }
        },
        trailingContent = {
            Text(stringResource(R.string.percent, dragged.toInt()), color = Brand.TextBright, style = MaterialTheme.typography.labelLarge)
        },
    )
}

@Composable
private fun Port(title: String, help: String, view: RangeView, set: (String, String) -> Unit) {
    var editing by remember { mutableStateOf(false) }
    ListItem(
        colors = rowColors,
        headlineContent = { Text(title) },
        supportingContent = { Text(help, style = MaterialTheme.typography.bodySmall) },
        trailingContent = {
            Text(view.value.toString(), color = Brand.TextBright, style = MaterialTheme.typography.labelLarge)
        },
        modifier = Modifier.clickable { editing = true },
    )
    if (editing) {
        var text by remember { mutableStateOf(view.value.toString()) }
        val port = text.toIntOrNull()?.takeIf { it in view.min.toInt()..view.max.toInt() }
        AlertDialog(
            onDismissRequest = { editing = false },
            title = { Text(title) },
            text = {
                CineoField(
                    value = text,
                    onValueChange = { text = it.filter(Char::isDigit).take(5) },
                    placeholder = "${view.min}–${view.max}",
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                )
            },
            confirmButton = {
                TextButton(onClick = { editing = false; set(view.key, port.toString()) }, enabled = port != null) {
                    Text(stringResource(R.string.save))
                }
            },
            dismissButton = {
                TextButton(onClick = { editing = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}

@Composable
private fun fontName(value: String) = when (value) {
    "sans" -> stringResource(R.string.font_sans)
    "serif" -> stringResource(R.string.font_serif)
    "mono" -> stringResource(R.string.font_mono)
    else -> value
}

@Composable
private fun colorName(value: String) = when (value) {
    "white" -> stringResource(R.string.color_white)
    "yellow" -> stringResource(R.string.color_yellow)
    "cyan" -> stringResource(R.string.color_cyan)
    "green" -> stringResource(R.string.color_green)
    else -> value
}

@Composable
private fun outlineName(value: String) = when (value) {
    "black" -> stringResource(R.string.outline_black)
    "gray" -> stringResource(R.string.outline_gray)
    "none" -> stringResource(R.string.outline_none)
    else -> value
}

@Composable
private fun backgroundName(value: String) = when (value) {
    "none" -> stringResource(R.string.background_none)
    "black" -> stringResource(R.string.background_black)
    "gray" -> stringResource(R.string.background_gray)
    else -> value
}

@Composable
private fun storageName(value: String) = when (value) {
    "ram-256" -> stringResource(R.string.storage_memory, "256 MB")
    "ram-512" -> stringResource(R.string.storage_memory, "512 MB")
    "ram-1024" -> stringResource(R.string.storage_memory, "1 GB")
    "ram-2048" -> stringResource(R.string.storage_memory, "2 GB")
    "disk-2048" -> stringResource(R.string.storage_disk, "2 GB")
    "disk" -> stringResource(R.string.storage_disk, "4 GB")
    "disk-8192" -> stringResource(R.string.storage_disk, "8 GB")
    else -> value
}

@Composable
private fun rateName(value: String) = when {
    value == "none" -> stringResource(R.string.no_limit)
    value.endsWith("k") -> "${value.dropLast(1)} kB/s"
    else -> "$value MB/s"
}

private const val SEARCHABLE_FROM = 12

@Composable
private fun <T> Picker(
    title: String,
    help: String,
    options: List<T>,
    current: T,
    label: @Composable (T) -> String,
    onPick: (T) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    ListItem(
        colors = rowColors,
        headlineContent = { Text(title) },
        supportingContent = { Text(help, style = MaterialTheme.typography.bodySmall) },
        trailingContent = {
            Row(
                Modifier.clip(RoundedCornerShape(50)).background(Brand.Surface).padding(start = 12.dp, end = 8.dp, top = 6.dp, bottom = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(label(current), color = Brand.TextBright, style = MaterialTheme.typography.labelLarge, maxLines = 1)
                CineoIcon(Icon.CHEVRON_DOWN, tint = Brand.TextDim, size = 16.sp)
            }
        },
        modifier = Modifier.clickable { open = true },
    )
    if (open) {
        val labelled = options.map { it to label(it) }
        PickerDialog(title, labelled, current, onDismiss = { open = false }) {
            open = false
            onPick(it)
        }
    }
}

@Composable
private fun <T> PickerDialog(
    title: String,
    options: List<Pair<T, String>>,
    current: T,
    onDismiss: () -> Unit,
    onPick: (T) -> Unit,
) {
    var filter by remember { mutableStateOf("") }
    val shown = if (filter.isBlank()) options else options.filter { it.second.contains(filter.trim(), ignoreCase = true) }
    val selectedIndex = shown.indexOfFirst { it.first == current }.coerceAtLeast(0)
    val list = rememberLazyListState(initialFirstVisibleItemIndex = (selectedIndex - 2).coerceAtLeast(0))
    Dialog(onDismissRequest = onDismiss) {
        Column(
            Modifier
                .fillMaxWidth()
                .heightIn(max = 560.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(Brand.Panel)
                .padding(vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                title,
                style = MaterialTheme.typography.titleLarge,
                color = Brand.TextBright,
                modifier = Modifier.padding(horizontal = 20.dp),
            )
            if (options.size >= SEARCHABLE_FROM) {
                CineoField(
                    value = filter,
                    onValueChange = { filter = it },
                    placeholder = stringResource(R.string.page_search),
                    leading = Icon.SEARCH,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                )
            }
            LazyColumn(state = list, modifier = Modifier.weight(1f, fill = false)) {
                items(shown.size) { index ->
                    val (option, text) = shown[index]
                    val selected = option == current
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable { onPick(option) }
                            .background(if (selected) Brand.Surface else Color.Transparent)
                            .padding(horizontal = 20.dp, vertical = 14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text,
                            modifier = Modifier.weight(1f),
                            style = MaterialTheme.typography.bodyLarge,
                            color = if (selected) Brand.Accent else Brand.TextBright,
                        )
                        if (selected) CineoIcon(Icon.CHECK, tint = Brand.Accent, size = 20.sp)
                    }
                }
            }
            TextButton(onClick = onDismiss, modifier = Modifier.align(Alignment.End).padding(end = 8.dp)) {
                Text(stringResource(R.string.cancel))
            }
        }
    }
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
        colors = rowColors,
        headlineContent = { Text(title) },
        supportingContent = { Text(help, style = MaterialTheme.typography.bodySmall) },
        trailingContent = {
            OutlinedButton(
                onClick = { asking = true },
                border = BorderStroke(1.dp, Brand.Danger.copy(alpha = 0.6f)),
            ) { Text(confirmLabel, color = Brand.Danger) }
        },
    )
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            title = { Text(confirmTitle) },
            text = { Text(confirmText) },
            confirmButton = {
                TextButton(onClick = { asking = false; onConfirm() }) { Text(confirmLabel, color = Brand.Danger) }
            },
            dismissButton = {
                TextButton(onClick = { asking = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}
