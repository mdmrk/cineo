package io.github.mdmrk.cineo

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.drawable.Icon
import android.os.Build
import android.os.IBinder
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import io.github.mdmrk.cineo.rust.ArtView
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.ImageException
import io.github.mdmrk.cineo.rust.PlaybackView
import io.github.mdmrk.cineo.rust.PlayerStatusView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.roundToInt

class TorrentService : Service() {
    private val scope = MainScope()
    private var session: PlaybackSession? = null
    private var torrent = true
    private var shown: Shown? = null
    private var artUrl: String? = null
    private var art: Bitmap? = null
    private var artJob: Job? = null

    private data class Shown(
        val torrent: Boolean,
        val title: String?,
        val paused: Boolean,
        val next: Boolean,
        val step: Int,
        val art: Bitmap?,
    )

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
                loadArt(cineo, playback?.art)
                session.update(playback, status, art)
                show(playback, status)
                delay(1000)
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            TOGGLE -> session?.togglePause()
            REWIND -> session?.seekBy(-1)
            FORWARD -> session?.seekBy(1)
            NEXT -> (application as CineoApplication).cineo?.playNext()
            else -> {
                torrent = intent?.getBooleanExtra(TORRENT, true) ?: true
                val kept = shown?.takeIf { it.title != null }?.copy(torrent = torrent)
                    ?: Shown(torrent, null, paused = false, next = false, step = 0, art = null)
                shown = kept
                ServiceCompat.startForeground(
                    this,
                    NOTIFICATION,
                    notification(kept),
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

    private fun loadArt(cineo: Cineo, view: ArtView?) {
        val url = view?.background ?: view?.logo
        if (url == artUrl) return
        artUrl = url
        art = null
        artJob?.cancel()
        if (url == null) return
        artJob = scope.launch {
            val bytes = try {
                cineo.fetchImage(url)
            } catch (e: ImageException) {
                return@launch
            }
            val decoded = withContext(Dispatchers.Default) { decodeArt(bytes) }
            if (artUrl == url) art = decoded
        }
    }

    private fun show(playback: PlaybackView?, status: PlayerStatusView?) {
        val playing = playback != null && status?.loaded == true
        val wanted = if (playing) {
            Shown(
                torrent,
                playback?.art?.title,
                status?.paused == true,
                playback?.next != null,
                playback?.seekStepS?.toInt() ?: 0,
                art,
            )
        } else {
            Shown(torrent, null, paused = false, next = false, step = 0, art = null)
        }
        if (wanted == shown) return
        shown = wanted
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION, notification(wanted))
    }

    private fun notification(shown: Shown): Notification {
        val channel = NotificationChannel(
            CHANNEL,
            getString(R.string.playback_channel),
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
            .setSmallIcon(if (shown.torrent) R.drawable.media_small_torrent else R.drawable.media_small)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setCategory(Notification.CATEGORY_TRANSPORT)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
        if (shown.title == null) {
            return builder.setContentTitle(text).build()
        }
        builder.setContentTitle(shown.title)
        if (shown.torrent) builder.setContentText(text)
        shown.art?.let { builder.setLargeIcon(it) }
        builder.addAction(action(PlaybackSession.backIcon(shown.step), R.string.seek_back_notification, REWIND))
        builder.addAction(
            if (shown.paused) action(R.drawable.media_play, R.string.play, TOGGLE)
            else action(R.drawable.media_pause, R.string.pause, TOGGLE),
        )
        builder.addAction(action(PlaybackSession.forwardIcon(shown.step), R.string.seek_forward_notification, FORWARD))
        if (shown.next) builder.addAction(action(R.drawable.media_next, R.string.next_episode, NEXT))
        val style = Notification.MediaStyle().setShowActionsInCompactView(0, 1, 2)
        session?.let { style.setMediaSession(it.token) }
        return builder.setStyle(style).build()
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
        private const val NEXT = "io.github.mdmrk.cineo.NEXT"
        private const val ART_SIDE = 512

        fun start(context: Context, torrent: Boolean) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, TorrentService::class.java).putExtra(TORRENT, torrent),
            )
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, TorrentService::class.java))
        }

        private fun decodeArt(bytes: ByteArray): Bitmap? {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
            var sample = 1
            while (maxOf(bounds.outWidth, bounds.outHeight) / (sample * 2) >= ART_SIDE) sample *= 2
            val decoded = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
                ?: return null
            val scale = ART_SIDE.toFloat() / maxOf(decoded.width, decoded.height)
            if (scale >= 1f) return decoded
            val width = (decoded.width * scale).roundToInt().coerceAtLeast(1)
            val height = (decoded.height * scale).roundToInt().coerceAtLeast(1)
            return Bitmap.createScaledBitmap(decoded, width, height, true)
        }
    }
}
