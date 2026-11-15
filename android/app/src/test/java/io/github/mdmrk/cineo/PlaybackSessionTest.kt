package io.github.mdmrk.cineo

import android.media.session.PlaybackState
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlaybackSessionTest {
    private val playing = Reported(PlaybackState.STATE_PLAYING, next = false, step = 10, positionMs = 60_000, atMs = 1_000)

    @Test
    fun aPositionThatAdvancesWithTheClockIsNotReportedAgain() {
        assertTrue(playing.stillHolds(playing.copy(positionMs = 65_200, atMs = 6_000)))
    }

    @Test
    fun aSeekOrAStallIsReportedAgain() {
        assertFalse(playing.stillHolds(playing.copy(positionMs = 95_000, atMs = 6_000)))
        assertFalse(playing.stillHolds(playing.copy(positionMs = 60_000, atMs = 6_000)))
    }

    @Test
    fun aPausedPositionHoldsUntilItMoves() {
        val paused = playing.copy(state = PlaybackState.STATE_PAUSED)
        assertTrue(paused.stillHolds(paused.copy(atMs = 90_000)))
        assertFalse(paused.stillHolds(paused.copy(positionMs = 70_000, atMs = 90_000)))
    }

    @Test
    fun aChangeOfStateOrButtonsIsReportedAgain() {
        assertFalse(playing.stillHolds(playing.copy(state = PlaybackState.STATE_PAUSED)))
        assertFalse(playing.stillHolds(playing.copy(next = true)))
        assertFalse(playing.stillHolds(playing.copy(step = 30)))
    }
}
