package io.github.mdmrk.cineo.ui

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.RemoteInputException
import io.github.mdmrk.cineo.rust.RemoteLinkView
import io.github.mdmrk.cineo.rust.RemotePeerView
import io.github.mdmrk.cineo.rust.remoteLink

private const val ADDRESS_HINT = "192.168.1.20:18765"
private const val CODE_HINT = "XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"

@Composable
internal fun RemoteSettings(
    peer: RemotePeerView?,
    link: RemoteLinkView?,
    onLinkUsed: () -> Unit,
    onOpen: () -> Unit,
    cineo: Cineo,
) {
    var address by rememberSaveable { mutableStateOf("") }
    var code by rememberSaveable { mutableStateOf("") }
    var error by remember { mutableStateOf<RemoteInputException?>(null) }
    var scanning by remember { mutableStateOf(false) }
    var another by rememberSaveable { mutableStateOf(false) }
    var cameraDenied by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val hasCamera = remember { context.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY) }
    val askCamera = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        cameraDenied = !granted
        scanning = granted
    }
    val pair = { newAddress: String, newCode: String ->
        error = try {
            cineo.remotePair(newAddress, newCode)
            address = ""
            code = ""
            another = false
            null
        } catch (e: RemoteInputException) {
            address = newAddress
            code = newCode
            e
        }
        error == null
    }
    LaunchedEffect(link) {
        if (link != null) {
            address = link.address
            code = link.code
            error = null
            another = true
            onLinkUsed()
        }
    }
    if (peer != null) {
        SettingRow(
            stringResource(R.string.remote_paired, peer.address),
            stringResource(if (peer.throughSync) R.string.remote_found_help else R.string.remote_paired_help),
        ) {}
        Divider()
    }
    if (peer != null && !another) {
        TextButton(onClick = { another = true }, modifier = Modifier.padding(horizontal = 8.dp)) {
            Text(stringResource(R.string.remote_pair_other), color = Brand.Accent)
        }
    } else ListItem(
        colors = rowColors,
        supportingContent = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.remote_pair_help), style = MaterialTheme.typography.bodySmall)
                CineoField(
                    value = address,
                    onValueChange = { address = it },
                    placeholder = ADDRESS_HINT,
                    modifier = Modifier.fillMaxWidth(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                )
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
                    OutlinedButton(onClick = { if (pair(address, code)) onOpen() }) {
                        Text(stringResource(R.string.sync_pair), color = Brand.Accent)
                    }
                }
            }
        },
    ) { Text(stringResource(if (peer == null) R.string.remote_pair_title else R.string.remote_pair_other)) }
    if (scanning) {
        QrScanner(
            hint = stringResource(R.string.remote_scan_hint),
            read = ::remoteLink,
            onFound = { found ->
                scanning = false
                cameraDenied = false
                if (pair(found.address, found.code)) onOpen()
            },
            onClose = { scanning = false },
        )
    }
    if (peer != null && !peer.throughSync) {
        Divider()
        SettingRow(stringResource(R.string.remote_forget), stringResource(R.string.remote_forget_help)) {
            OutlinedButton(onClick = cineo::remoteForget) { Text(stringResource(R.string.remote_forget_button), color = Brand.Danger) }
        }
    }
}

@Composable
private fun inputError(error: RemoteInputException): String = stringResource(
    when (error) {
        is RemoteInputException.Address -> R.string.remote_error_address
        is RemoteInputException.CodeLength -> R.string.remote_error_length
        is RemoteInputException.CodeTypo -> R.string.sync_error_typo
        is RemoteInputException.NotACode -> R.string.remote_error_code
    },
)
