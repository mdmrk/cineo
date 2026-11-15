package io.github.mdmrk.cineo

import android.media.AudioManager
import org.junit.Assert.assertEquals
import org.junit.Test

class PlaybackFocusTest {
    private val held = FocusState(held = true)

    @Test
    fun aShortLossPausesAndPlaysAgainWhenTheFocusReturns() {
        val (lost, pause) = held.on(AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, playing = true)
        assertEquals(FocusOrder.PAUSE, pause)
        val (back, play) = lost.on(AudioManager.AUDIOFOCUS_GAIN, playing = false)
        assertEquals(FocusOrder.PLAY, play)
        assertEquals(held, back)
    }

    @Test
    fun aVideoThatWasPausedBeforeAShortLossStaysPaused() {
        val (lost, _) = held.on(AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, playing = false)
        assertEquals(null, lost.on(AudioManager.AUDIOFOCUS_GAIN, playing = false).second)
    }

    @Test
    fun aSecondShortLossWhilePausedByTheFirstStillPlaysAgain() {
        val (once, _) = held.on(AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, playing = true)
        val (twice, _) = once.on(AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, playing = false)
        assertEquals(FocusOrder.PLAY, twice.on(AudioManager.AUDIOFOCUS_GAIN, playing = false).second)
    }

    @Test
    fun aLastingLossPausesAndGivesUpTheFocusForGood() {
        val (lost, pause) = held.on(AudioManager.AUDIOFOCUS_LOSS, playing = true)
        assertEquals(FocusOrder.PAUSE, pause)
        assertEquals(FocusState(), lost)
        assertEquals(null, lost.on(AudioManager.AUDIOFOCUS_GAIN, playing = false).second)
    }

    @Test
    fun duckingIsLeftToTheSystem() {
        assertEquals(held to null, held.on(AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK, playing = true))
    }
}
