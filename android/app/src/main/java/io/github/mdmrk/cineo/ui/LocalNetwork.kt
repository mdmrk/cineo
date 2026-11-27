package io.github.mdmrk.cineo.ui

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.LocalActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LifecycleResumeEffect
import io.github.mdmrk.cineo.R

private const val LOCAL_NETWORK = "android.permission.ACCESS_LOCAL_NETWORK"

private fun localNetworkAllowed(context: Context) =
    Build.VERSION.SDK_INT < Build.VERSION_CODES.CINNAMON_BUN ||
        ContextCompat.checkSelfPermission(context, LOCAL_NETWORK) == PackageManager.PERMISSION_GRANTED

@Composable
internal fun PrivateNetwork(title: String, help: String, on: Boolean, onChange: (Boolean) -> Unit) {
    val context = LocalContext.current
    val activity = LocalActivity.current
    var allowed by remember { mutableStateOf(localNetworkAllowed(context)) }
    var refused by remember { mutableStateOf(false) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        allowed = granted
        refused = !granted
    }
    LifecycleResumeEffect(Unit) {
        allowed = localNetworkAllowed(context)
        onPauseOrDispose {}
    }
    val change = { value: Boolean ->
        onChange(value)
        if (value && !allowed) ask.launch(LOCAL_NETWORK)
    }
    ListItem(
        colors = rowColors,
        modifier = Modifier.clickable { change(!on) },
        supportingContent = {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(help, style = MaterialTheme.typography.bodySmall)
                if (on && !allowed) {
                    Text(
                        stringResource(R.string.private_network_blocked),
                        color = Brand.Warning,
                        style = MaterialTheme.typography.bodySmall,
                    )
                    TextButton(onClick = {
                        if (refused && activity != null &&
                            !ActivityCompat.shouldShowRequestPermissionRationale(activity, LOCAL_NETWORK)
                        ) {
                            context.startActivity(
                                Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null)),
                            )
                        } else {
                            ask.launch(LOCAL_NETWORK)
                        }
                    }) { Text(stringResource(R.string.private_network_allow), color = Brand.Accent) }
                }
            }
        },
        trailingContent = {
            Switch(
                checked = on,
                onCheckedChange = change,
                colors = SwitchDefaults.colors(checkedTrackColor = Brand.Accent, checkedThumbColor = Brand.OnAccent),
            )
        },
    ) { Text(title) }
}
