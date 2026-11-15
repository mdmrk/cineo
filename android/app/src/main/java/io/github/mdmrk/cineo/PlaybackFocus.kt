package io.github.mdmrk.cineo

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager

internal class PlaybackFocus(context: Context, private val player: Player) :
    AudioManager.OnAudioFocusChangeListener {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MOVIE)
                .build(),
        )
        .setOnAudioFocusChangeListener(this)
        .build()
    private var state = FocusState()

    fun playing() {
        if (!state.held) {
            state = FocusState(held = audio.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED)
        }
    }

    fun abandon() {
        audio.abandonAudioFocusRequest(request)
        state = FocusState()
    }

    override fun onAudioFocusChange(change: Int) {
        val (next, order) = state.on(change, player.status.value?.paused == false)
        state = next
        when (order) {
            FocusOrder.PAUSE -> player.pause()
            FocusOrder.PLAY -> player.play()
            null -> {}
        }
    }
}

internal enum class FocusOrder { PAUSE, PLAY }

internal data class FocusState(val held: Boolean = false, val resume: Boolean = false) {
    fun on(change: Int, playing: Boolean): Pair<FocusState, FocusOrder?> = when (change) {
        AudioManager.AUDIOFOCUS_LOSS -> FocusState() to FocusOrder.PAUSE
        AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> copy(resume = resume || playing) to FocusOrder.PAUSE
        AudioManager.AUDIOFOCUS_GAIN -> copy(held = true, resume = false) to FocusOrder.PLAY.takeIf { resume }
        else -> this to null
    }
}
