package io.github.mdmrk.cineo

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.os.Build
import android.os.IBinder
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import io.github.mdmrk.cineo.rust.PlayerStatusView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class TorrentService : Service() {
    private val scope = MainScope()
    private var session: PlaybackSession? = null
    private var torrent = true
    private var shown: Shown? = null

    private data class Shown(val torrent: Boolean, val title: String?, val paused: Boolean, val next: Boolean)

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val app = application as CineoApplication
        val cineo = app.cineo ?: return
        val session = PlaybackSession(this, cineo).also { session = it }
        scope.launch {
            while (isActive) {
                val status = withContext(Dispatchers.Default) { cineo.playerStatus() }
                val playback = app.view.value?.playback
                session.update(playback, status)
                show(playback?.art?.title, status, playback?.next != null)
                delay(1000)
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            TOGGLE -> session?.togglePause()
            REWIND -> session?.seekBy(-1)
            FORWARD -> session?.seekBy(1)
            else -> {
                torrent = intent?.getBooleanExtra(TORRENT, true) ?: true
                val plain = Shown(torrent, null, paused = false, next = false)
                shown = plain
                ServiceCompat.startForeground(
                    this,
                    NOTIFICATION,
                    notification(plain),
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
                )
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        session?.release()
        super.onDestroy()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        (application as CineoApplication).cineo?.stopPlayback()
        stopSelf()
    }

    private fun show(title: String?, status: PlayerStatusView?, next: Boolean) {
        val playing = status?.loaded == true
        val wanted = Shown(torrent, title.takeIf { playing }, status?.paused == true, playing && next)
        if (wanted == shown) return
        shown = wanted
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION, notification(wanted))
    }

    private fun notification(shown: Shown): Notification {
        val channel = NotificationChannel(
            CHANNEL,
            getString(R.string.torrent_channel),
            NotificationManager.IMPORTANCE_LOW,
        )
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val text = getString(if (shown.torrent) R.string.torrent_notification else R.string.playback_notification)
        val builder = Notification.Builder(this, CHANNEL)
            .setSmallIcon(if (shown.torrent) android.R.drawable.stat_sys_download else android.R.drawable.ic_media_play)
            .setContentTitle(shown.title ?: text)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
        if (shown.title != null) {
            builder.setContentText(text)
            builder.addAction(action(android.R.drawable.ic_media_rew, R.string.seek_back_notification, REWIND))
            builder.addAction(
                if (shown.paused) action(android.R.drawable.ic_media_play, R.string.play, TOGGLE)
                else action(android.R.drawable.ic_media_pause, R.string.pause, TOGGLE),
            )
            builder.addAction(action(android.R.drawable.ic_media_ff, R.string.seek_forward_notification, FORWARD))
            val style = Notification.MediaStyle().setShowActionsInCompactView(0, 1, 2)
            session?.let { style.setMediaSession(it.token) }
            builder.setStyle(style)
        }
        return builder.build()
    }

    private fun action(icon: Int, label: Int, name: String): Notification.Action {
        val intent = PendingIntent.getService(
            this,
            name.hashCode(),
            Intent(this, TorrentService::class.java).setAction(name),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Action.Builder(Icon.createWithResource(this, icon), getString(label), intent).build()
    }

    companion object {
        private const val CHANNEL = "torrents"
        private const val NOTIFICATION = 1
        private const val TORRENT = "torrent"
        private const val TOGGLE = "io.github.mdmrk.cineo.TOGGLE"
        private const val REWIND = "io.github.mdmrk.cineo.REWIND"
        private const val FORWARD = "io.github.mdmrk.cineo.FORWARD"

        fun start(context: Context, torrent: Boolean) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, TorrentService::class.java).putExtra(TORRENT, torrent),
            )
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, TorrentService::class.java))
        }
    }
}
