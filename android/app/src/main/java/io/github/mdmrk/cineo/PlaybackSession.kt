package io.github.mdmrk.cineo

import android.content.Context
import android.graphics.Bitmap
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Bundle
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.PlaybackView
import io.github.mdmrk.cineo.rust.PlayerStatusView

internal class PlaybackSession(private val context: Context, private val cineo: Cineo) {
    private val session = MediaSession(context, "Cineo")
    private var playback: PlaybackView? = null
    private var paused = false
    private var metadata: Triple<String, Long, Bitmap?>? = null

    val token: MediaSession.Token get() = session.sessionToken

    init {
        session.setCallback(
            object : MediaSession.Callback() {
                override fun onPlay() {
                    if (paused) cineo.togglePause()
                }

                override fun onPause() {
                    cineo.pause()
                }

                override fun onStop() {
                    cineo.stopPlayback()
                }

                override fun onSeekTo(pos: Long) {
                    cineo.seekTo(pos / 1000.0)
                }

                override fun onFastForward() {
                    seekBy(1)
                }

                override fun onRewind() {
                    seekBy(-1)
                }

                override fun onSkipToNext() {
                    if (playback?.next != null) cineo.playNext()
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
        cineo.seekBy((direction * step).toDouble())
    }

    fun togglePause() {
        cineo.togglePause()
    }

    fun update(playback: PlaybackView?, status: PlayerStatusView?, art: Bitmap?) {
        this.playback = playback
        if (playback == null || status == null || !status.loaded) {
            session.isActive = false
            return
        }
        paused = status.paused
        val durationMs = (status.durationS * 1000).toLong()
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
        val state = when {
            status.paused -> PlaybackState.STATE_PAUSED
            status.waiting -> PlaybackState.STATE_BUFFERING
            else -> PlaybackState.STATE_PLAYING
        }
        val next = if (playback.next != null) PlaybackState.ACTION_SKIP_TO_NEXT else 0L
        val step = playback.seekStepS.toInt()
        session.setPlaybackState(
            PlaybackState.Builder()
                .setActions(ACTIONS or next)
                .setState(state, (status.positionS * 1000).toLong(), if (state == PlaybackState.STATE_PLAYING) 1f else 0f)
                .addCustomAction(REWIND, context.getString(R.string.seek_back_notification), backIcon(step))
                .addCustomAction(FORWARD, context.getString(R.string.seek_forward_notification), forwardIcon(step))
                .build(),
        )
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
