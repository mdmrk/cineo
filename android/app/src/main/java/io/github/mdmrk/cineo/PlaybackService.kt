package io.github.mdmrk.cineo

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.drawable.Icon
import android.media.AudioManager
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.core.graphics.scale
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.ImageException
import io.github.mdmrk.cineo.rust.PlayerStatusView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.roundToInt

class PlaybackService : Service() {
    private val scope = MainScope()
    private val art = MutableStateFlow<Bitmap?>(null)
    private var player: Player? = null
    private var session: PlaybackSession? = null
    private var focus: PlaybackFocus? = null
    private var shown: Shown? = null
    private var artUrl: String? = null
    private var artJob: Job? = null
    private val wake by lazy {
        getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "cineo:playback")
            .apply { setReferenceCounted(false) }
    }
    private val wifi by lazy {
        applicationContext.getSystemService(WifiManager::class.java)
            .createWifiLock(wifiMode(), "cineo:playback")
            .apply { setReferenceCounted(false) }
    }
    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            player?.pause()
        }
    }

    private data class Shown(
        val torrent: Boolean,
        val title: String?,
        val controls: Boolean,
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
        val player = app.player ?: return
        this.player = player
        session = PlaybackSession(this, player)
        focus = PlaybackFocus(this, player)
        ContextCompat.registerReceiver(
            this,
            noisy,
            IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        scope.launch {
            combine(app.view, player.status, art) { view, status, art -> Triple(view, status, art) }
                .collect { (view, status, art) -> render(cineo, view, status, art) }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            TOGGLE -> player?.togglePause()
            REWIND -> session?.seekBy(-1)
            FORWARD -> session?.seekBy(1)
            NEXT -> player?.next()
            else -> {
                val app = application as CineoApplication
                val now = shown(app.view.value, player?.status?.value, art.value)
                shown = now
                ServiceCompat.startForeground(
                    this,
                    NOTIFICATION,
                    notification(now),
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
                )
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        if (player != null) unregisterReceiver(noisy)
        focus?.abandon()
        session?.release()
        keepAwake(false)
        super.onDestroy()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        player?.stop()
        stopSelf()
    }

    private fun render(cineo: Cineo, view: AppView?, status: PlayerStatusView?, art: Bitmap?) {
        val playback = view?.playback
        if (playback == null && view?.connecting == null && view?.torrent == null) {
            stopSelf()
            return
        }
        loadArt(cineo, (playback?.art ?: view.connecting)?.let { it.background ?: it.logo })
        val reported = session?.update(playback, status, art) == true
        if (status?.loaded == true && !status.paused) focus?.playing()
        keepAwake(status?.paused != true)
        val now = shown(view, status, art)
        if (now != shown || reported) {
            shown = now
            getSystemService(NotificationManager::class.java).notify(NOTIFICATION, notification(now))
        }
    }

    private fun keepAwake(awake: Boolean) {
        if (awake) {
            wake.acquire(AWAKE_LEASE_MS)
            wifi.acquire()
        } else {
            if (wake.isHeld) wake.release()
            if (wifi.isHeld) wifi.release()
        }
    }

    private fun loadArt(cineo: Cineo, url: String?) {
        if (url == artUrl) return
        artUrl = url
        art.value = null
        artJob?.cancel()
        if (url == null) return
        artJob = scope.launch {
            val bytes = try {
                cineo.fetchImage(url)
            } catch (e: ImageException) {
                return@launch
            }
            art.value = withContext(Dispatchers.Default) { decodeArt(bytes) }
        }
    }

    private fun shown(view: AppView?, status: PlayerStatusView?, art: Bitmap?): Shown {
        val playback = view?.playback
        return Shown(
            torrent = view?.torrent != null,
            title = (playback?.art ?: view?.connecting)?.title,
            controls = playback != null,
            paused = status?.paused == true,
            next = playback?.next != null,
            step = playback?.seekStepS?.toInt() ?: 0,
            art = art,
        )
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
            .setContentTitle(shown.title ?: text)
        if (shown.title != null && shown.torrent) builder.setContentText(text)
        shown.art?.let { builder.setLargeIcon(it) }
        if (!shown.controls) return builder.build()
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

    private fun action(icon: Int, label: Int, name: String): Notification.Action =
        Notification.Action.Builder(Icon.createWithResource(this, icon), getString(label), order(this, name)).build()

    companion object {
        const val TOGGLE = "io.github.mdmrk.cineo.TOGGLE"
        const val REWIND = "io.github.mdmrk.cineo.REWIND"
        const val FORWARD = "io.github.mdmrk.cineo.FORWARD"
        private const val NEXT = "io.github.mdmrk.cineo.NEXT"
        private const val CHANNEL = "torrents"
        private const val NOTIFICATION = 1
        private const val ART_SIDE = 512
        private const val AWAKE_LEASE_MS = 10 * 60 * 1000L

        fun start(context: Context) {
            ContextCompat.startForegroundService(context, Intent(context, PlaybackService::class.java))
        }

        fun order(context: Context, name: String): PendingIntent = PendingIntent.getService(
            context,
            name.hashCode(),
            Intent(context, PlaybackService::class.java).setAction(name),
            PendingIntent.FLAG_IMMUTABLE,
        )

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
            return decoded.scale(width, height)
        }
    }
}

@Suppress("DEPRECATION")
private fun wifiMode() =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) WifiManager.WIFI_MODE_FULL_LOW_LATENCY
    else WifiManager.WIFI_MODE_FULL_HIGH_PERF
