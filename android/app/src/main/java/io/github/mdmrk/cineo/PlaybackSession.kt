package io.github.mdmrk.cineo

import android.content.Context
import android.graphics.Bitmap
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Bundle
import android.os.SystemClock
import io.github.mdmrk.cineo.rust.PlaybackView
import io.github.mdmrk.cineo.rust.PlayerStatusView
import kotlin.math.abs

internal class PlaybackSession(private val context: Context, private val player: Player) {
    private val session = MediaSession(context, "Cineo")
    private var playback: PlaybackView? = null
    private var metadata: Triple<String, Long, Bitmap?>? = null
    private var reported: Reported? = null

    val token: MediaSession.Token get() = session.sessionToken

    init {
        session.setCallback(
            object : MediaSession.Callback() {
                override fun onPlay() = player.play()

                override fun onPause() = player.pause()

                override fun onStop() = player.stop()

                override fun onSeekTo(pos: Long) = player.seekTo(pos / 1000.0)

                override fun onFastForward() = seekBy(1)

                override fun onRewind() = seekBy(-1)

                override fun onSkipToNext() {
                    if (playback?.next != null) player.next()
                }

                override fun onCustomAction(action: String, extras: Bundle?) {
                    when (action) {
                        REWIND -> seekBy(-1)
                        FORWARD -> seekBy(1)
                    }
                }
            },
        )
    }

    fun seekBy(direction: Int) {
        val step = playback?.seekStepS?.toInt() ?: return
        player.seekBy((direction * step).toDouble())
    }

    fun update(playback: PlaybackView?, status: PlayerStatusView?, art: Bitmap?) {
        this.playback = playback
        if (playback == null) {
            session.isActive = false
            reported = null
            return
        }
        val durationMs = ((status?.durationS ?: 0.0) * 1000).toLong()
        val wanted = Triple(playback.art.title, durationMs, art)
        if (metadata != wanted) {
            metadata = wanted
            session.setMetadata(
                MediaMetadata.Builder()
                    .putString(MediaMetadata.METADATA_KEY_TITLE, playback.art.title)
                    .putString(MediaMetadata.METADATA_KEY_DISPLAY_TITLE, playback.art.title)
                    .putLong(MediaMetadata.METADATA_KEY_DURATION, durationMs)
                    .apply { if (art != null) putBitmap(MediaMetadata.METADATA_KEY_ART, art) }
                    .build(),
            )
        }
        val now = Reported(
            state = when {
                status == null || !status.loaded || status.waiting -> PlaybackState.STATE_BUFFERING
                status.paused -> PlaybackState.STATE_PAUSED
                else -> PlaybackState.STATE_PLAYING
            },
            next = playback.next != null,
            step = playback.seekStepS.toInt(),
            positionMs = ((status?.positionS ?: 0.0) * 1000).toLong(),
            atMs = SystemClock.elapsedRealtime(),
        )
        if (reported?.stillHolds(now) != true) {
            reported = now
            session.setPlaybackState(
                PlaybackState.Builder()
                    .setActions(ACTIONS or if (now.next) PlaybackState.ACTION_SKIP_TO_NEXT else 0L)
                    .setState(now.state, now.positionMs, if (now.state == PlaybackState.STATE_PLAYING) 1f else 0f, now.atMs)
                    .addCustomAction(REWIND, context.getString(R.string.seek_back_notification), backIcon(now.step))
                    .addCustomAction(FORWARD, context.getString(R.string.seek_forward_notification), forwardIcon(now.step))
                    .build(),
            )
        }
        session.isActive = true
    }

    fun release() {
        session.isActive = false
        session.release()
    }

    companion object {
        private const val REWIND = "io.github.mdmrk.cineo.session.REWIND"
        private const val FORWARD = "io.github.mdmrk.cineo.session.FORWARD"
        private const val ACTIONS = PlaybackState.ACTION_PLAY or
            PlaybackState.ACTION_PAUSE or
            PlaybackState.ACTION_PLAY_PAUSE or
            PlaybackState.ACTION_STOP or
            PlaybackState.ACTION_SEEK_TO or
            PlaybackState.ACTION_FAST_FORWARD or
            PlaybackState.ACTION_REWIND

        fun backIcon(step: Int) = when (step) {
            5 -> R.drawable.media_back_5
            15 -> R.drawable.media_back_15
            30 -> R.drawable.media_back_30
            else -> R.drawable.media_back_10
        }

        fun forwardIcon(step: Int) = when (step) {
            5 -> R.drawable.media_forward_5
            15 -> R.drawable.media_forward_15
            30 -> R.drawable.media_forward_30
            else -> R.drawable.media_forward_10
        }
    }
}

internal data class Reported(
    val state: Int,
    val next: Boolean,
    val step: Int,
    val positionMs: Long,
    val atMs: Long,
) {
    fun stillHolds(now: Reported): Boolean {
        if (copy(positionMs = now.positionMs, atMs = now.atMs) != now) return false
        val moved = if (state == PlaybackState.STATE_PLAYING) now.atMs - atMs else 0L
        return abs(now.positionMs - (positionMs + moved)) <= DRIFT_MS
    }

    private companion object {
        const val DRIFT_MS = 1500L
    }
}
