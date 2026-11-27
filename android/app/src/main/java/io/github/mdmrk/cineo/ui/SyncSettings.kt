package io.github.mdmrk.cineo.ui

import android.content.ClipData
import android.content.ClipDescription
import android.Manifest
import android.content.ClipboardManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.PersistableBundle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.QrView
import io.github.mdmrk.cineo.rust.SyncFailureView
import io.github.mdmrk.cineo.rust.SyncInputException
import io.github.mdmrk.cineo.rust.SyncLinkView
import io.github.mdmrk.cineo.rust.SyncStatusView
import io.github.mdmrk.cineo.rust.SyncView
import io.github.mdmrk.cineo.rust.defaultSyncServer
import io.github.mdmrk.cineo.rust.syncLink

private const val QR_QUIET = 4
private const val QR_MODULE_DP = 4
private const val CODE_HINT = "XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"

@Composable
internal fun SyncSettings(sync: SyncView?, link: SyncLinkView?, onLinkUsed: () -> Unit, cineo: Cineo) {
    var code by rememberSaveable { mutableStateOf("") }
    var server by rememberSaveable { mutableStateOf("") }
    var error by remember { mutableStateOf<SyncInputException?>(null) }
    var scanning by remember { mutableStateOf(false) }
    var cameraDenied by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val hasCamera = remember { context.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY) }
    val askCamera = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        cameraDenied = !granted
        scanning = granted
    }
    LaunchedEffect(link) {
        if (link != null) {
            if (sync == null) {
                code = link.code
                server = link.server
                error = null
            }
            onLinkUsed()
        }
    }
    val attempt = { action: () -> Unit ->
        error = try {
            action()
            null
        } catch (e: SyncInputException) {
            e
        }
    }
    if (sync == null) {
        SettingRow(stringResource(R.string.sync), stringResource(R.string.sync_help)) {
            OutlinedButton(onClick = { attempt { cineo.syncTurnOn(server) } }) {
                Text(stringResource(R.string.sync_turn_on), color = Brand.Accent)
            }
        }
        Divider()
        Field(stringResource(R.string.sync_pair_title), stringResource(R.string.sync_pair_help)) {
            CineoField(
                value = code,
                onValueChange = { code = it },
                placeholder = CODE_HINT,
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Characters,
                    keyboardType = KeyboardType.Ascii,
                ),
            )
            error?.let { Text(inputError(it), color = Brand.Warning, style = MaterialTheme.typography.bodySmall) }
            if (cameraDenied) {
                Text(stringResource(R.string.sync_scan_denied), color = Brand.Warning, style = MaterialTheme.typography.bodySmall)
            }
            Row(Modifier.align(Alignment.End), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (hasCamera) {
                    OutlinedButton(onClick = {
                        val granted = ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                            PackageManager.PERMISSION_GRANTED
                        if (granted) scanning = true else askCamera.launch(Manifest.permission.CAMERA)
                    }) { Text(stringResource(R.string.sync_scan), color = Brand.Accent) }
                }
                OutlinedButton(
                    onClick = { attempt { cineo.syncPair(code, server); code = "" } },
                ) { Text(stringResource(R.string.sync_pair), color = Brand.Accent) }
            }
        }
        if (scanning) {
            QrScanner(
                hint = stringResource(R.string.sync_scan_hint),
                read = ::syncLink,
                onFound = { link ->
                    code = link.code
                    server = link.server
                    error = null
                    cameraDenied = false
                    scanning = false
                },
                onClose = { scanning = false },
            )
        }
        Divider()
        Field(stringResource(R.string.sync_server), stringResource(R.string.sync_server_help)) {
            CineoField(
                value = server,
                onValueChange = { server = it },
                placeholder = remember { defaultSyncServer() },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            )
        }
    } else {
        SyncOn(sync, cineo, error, attempt)
    }
}

@Composable
private fun SyncOn(sync: SyncView, cineo: Cineo, error: SyncInputException?, attempt: (() -> Unit) -> Unit) {
    val context = LocalContext.current
    SettingRow(stringResource(R.string.sync_on), status(sync.status)) {
        OutlinedButton(onClick = cineo::syncNow, enabled = sync.canSyncNow) {
            Text(stringResource(R.string.sync_now))
        }
    }
    Divider()
    Field(stringResource(R.string.sync_code), stringResource(R.string.sync_code_help)) {
        sync.qr?.let { Qr(it) }
        Text(
            sync.code,
            color = Brand.TextBright,
            fontFamily = FontFamily.Monospace,
            style = MaterialTheme.typography.bodyLarge,
        )
        TextButton(onClick = { copy(context, sync.code) }, modifier = Modifier.align(Alignment.End)) {
            Text(stringResource(R.string.sync_copy), color = Brand.Accent)
        }
    }
    Divider()
    Field(stringResource(R.string.sync_change), stringResource(R.string.sync_change_help)) {
        error?.let { Text(inputError(it), color = Brand.Warning, style = MaterialTheme.typography.bodySmall) }
        AskFirst(
            stringResource(R.string.sync_change_button),
            stringResource(R.string.sync_change_title),
            stringResource(R.string.sync_change_text),
            stringResource(R.string.sync_change_confirm),
            Modifier.align(Alignment.End),
        ) { attempt { cineo.syncChangeKey() } }
    }
    Divider()
    Field(stringResource(R.string.sync_off), stringResource(R.string.sync_off_help)) {
        Row(Modifier.align(Alignment.End), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            AskFirst(
                stringResource(R.string.sync_delete_button),
                stringResource(R.string.sync_delete_title),
                stringResource(R.string.sync_delete_text),
                stringResource(R.string.sync_delete_confirm),
            ) { cineo.syncTurnOff(true) }
            AskFirst(
                stringResource(R.string.sync_off_button),
                stringResource(R.string.sync_off_title),
                stringResource(R.string.sync_off_text),
                stringResource(R.string.sync_off_confirm),
            ) { cineo.syncTurnOff(false) }
        }
    }
}

@Composable
private fun Field(title: String, help: String, content: @Composable ColumnScope.() -> Unit) {
    ListItem(
        colors = rowColors,
        supportingContent = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(help, style = MaterialTheme.typography.bodySmall)
                content()
            }
        },
    ) { Text(title) }
}

@Composable
private fun AskFirst(
    label: String,
    title: String,
    text: String,
    confirm: String,
    modifier: Modifier = Modifier,
    onConfirm: () -> Unit,
) {
    var asking by remember { mutableStateOf(false) }
    OutlinedButton(
        onClick = { asking = true },
        modifier = modifier,
        border = BorderStroke(1.dp, Brand.Danger.copy(alpha = 0.6f)),
    ) { Text(label, color = Brand.Danger) }
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            title = { Text(title) },
            text = { Text(text) },
            confirmButton = {
                TextButton(onClick = { asking = false; onConfirm() }) { Text(confirm, color = Brand.Danger) }
            },
            dismissButton = {
                TextButton(onClick = { asking = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}

@Composable
private fun Qr(qr: QrView) {
    val width = qr.width.toInt()
    val modules = width + 2 * QR_QUIET
    val description = stringResource(R.string.sync_qr)
    Canvas(
        Modifier
            .padding(vertical = 8.dp)
            .size((modules * QR_MODULE_DP).dp)
            .semantics { contentDescription = description },
    ) {
        val module = size.width / modules
        drawRect(Color.White)
        qr.dark.forEachIndexed { index, dark ->
            if (dark) {
                drawRect(
                    Color.Black,
                    topLeft = Offset((index % width + QR_QUIET) * module, (index / width + QR_QUIET) * module),
                    size = Size(module, module),
                )
            }
        }
    }
}

private fun copy(context: Context, code: String) {
    val clip = ClipData.newPlainText("Cineo", code)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        clip.description.extras = PersistableBundle().apply {
            putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true)
        }
    }
    context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(clip)
}

@Composable
private fun status(status: SyncStatusView): String = when (status) {
    SyncStatusView.Syncing -> stringResource(R.string.sync_syncing)
    SyncStatusView.NotYet -> stringResource(R.string.sync_not_yet)
    is SyncStatusView.Synced -> if (status.minutesAgo == 0UL) {
        stringResource(R.string.sync_just_now)
    } else {
        stringResource(R.string.sync_minutes_ago, status.minutesAgo.toLong())
    }
    SyncStatusView.KeyChanged -> stringResource(R.string.sync_key_changed)
    SyncStatusView.Busy -> stringResource(R.string.sync_busy)
    is SyncStatusView.Failed -> stringResource(R.string.sync_failed, failure(status.reason))
}

@Composable
private fun failure(reason: SyncFailureView): String = when (reason) {
    SyncFailureView.Insecure -> stringResource(R.string.sync_failed_insecure)
    SyncFailureView.Unreachable -> stringResource(R.string.sync_failed_unreachable)
    is SyncFailureView.Rejected -> stringResource(R.string.sync_failed_rejected, reason.status.toInt())
    SyncFailureView.Unreadable -> stringResource(R.string.sync_failed_unreadable)
    SyncFailureView.Conflict -> stringResource(R.string.sync_failed_conflict)
    SyncFailureView.Unavailable -> stringResource(R.string.sync_failed_unavailable)
}

@Composable
private fun inputError(error: SyncInputException): String = stringResource(
    when (error) {
        is SyncInputException.CodeLength -> R.string.sync_error_length
        is SyncInputException.CodeTypo -> R.string.sync_error_typo
        is SyncInputException.NotACode -> R.string.sync_error_code
        is SyncInputException.Server -> R.string.sync_error_server
        is SyncInputException.Random -> R.string.sync_error_random
    },
)
