package io.github.mdmrk.cineo

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.core.content.edit
import io.github.mdmrk.cineo.rust.UpdateView

@Composable
fun UpdateNotification(update: UpdateView?, canAsk: Boolean) {
    val context = LocalContext.current
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted && update != null) UpdateNotice.post(context, update)
    }
    LaunchedEffect(update?.version, canAsk) {
        if (update == null || UpdateNotice.posted(context, update.version)) return@LaunchedEffect
        val allowed = Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
        if (!allowed && !canAsk) return@LaunchedEffect
        UpdateNotice.markPosted(context, update.version)
        if (allowed) UpdateNotice.post(context, update) else ask.launch(Manifest.permission.POST_NOTIFICATIONS)
    }
}

private object UpdateNotice {
    private const val CHANNEL = "updates"
    private const val NOTIFICATION = 2
    private const val PREFERENCES = "updates"
    private const val POSTED = "posted_version"

    fun posted(context: Context, version: String) =
        context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).getString(POSTED, null) == version

    fun markPosted(context: Context, version: String) {
        context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).edit { putString(POSTED, version) }
    }

    fun post(context: Context, update: UpdateView) {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, context.getString(R.string.update_channel), NotificationManager.IMPORTANCE_DEFAULT),
        )
        val open = PendingIntent.getActivity(
            context,
            NOTIFICATION,
            Intent(Intent.ACTION_VIEW, Uri.parse(update.pageUrl)),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setContentTitle(context.getString(R.string.update_notification_title, update.version))
            .setContentText(context.getString(R.string.update_notification_text))
            .setContentIntent(open)
            .setAutoCancel(true)
            .build()
        manager.notify(NOTIFICATION, notification)
    }
}
