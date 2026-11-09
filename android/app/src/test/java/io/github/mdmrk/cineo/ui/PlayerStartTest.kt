package io.github.mdmrk.cineo.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerStartTest {
    @Test
    fun aPlayerThatIsStillLoadingOrBufferingHasNotStarted() {
        assertFalse(playerStarted(was = false, loaded = false, waiting = false))
        assertFalse(playerStarted(was = false, loaded = true, waiting = true))
    }

    @Test
    fun aLoadedPlayerThatIsNotWaitingHasStarted() {
        assertTrue(playerStarted(was = false, loaded = true, waiting = false))
    }

    @Test
    fun bufferingAfterTheStartKeepsItStarted() {
        assertTrue(playerStarted(was = true, loaded = true, waiting = true))
    }

    @Test
    fun aNewVideoThatIsNotLoadedStartsOver() {
        assertFalse(playerStarted(was = true, loaded = false, waiting = false))
    }
}
