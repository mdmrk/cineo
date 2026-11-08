package io.github.mdmrk.cineo.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SeekHoldTest {
    @Test
    fun aDragKeepsItsValueUntilThePlayerCatchesUp() {
        val dragged = SeekHold().dragTo(90.0)
        assertEquals(90.0, dragged.settle(10.0, busy = false, nowMs = 5000).value)
        val sent = dragged.commit(90.0, nowMs = 0)
        assertEquals(90.0, sent.settle(10.0, busy = false, nowMs = 250).value)
        assertNull(sent.settle(89.5, busy = false, nowMs = 500).value)
    }

    @Test
    fun aHeldSeekEndsWhenThePlayerFinishesSeekingOrTooLongPasses() {
        val sent = SeekHold().commit(90.0, nowMs = 0)
        val seeking = sent.settle(10.0, busy = true, nowMs = 100)
        assertEquals(90.0, seeking.value)
        assertNull(seeking.settle(86.0, busy = false, nowMs = 200).value)
        assertNull(sent.settle(10.0, busy = false, nowMs = 2100).value)
    }

    @Test
    fun relativeSeeksAddUpFromThePendingTargetWithinTheMedia() {
        val once = SeekHold().commitBy(10.0, position = 50.0, duration = 100.0, nowMs = 0)
        val twice = once.commitBy(10.0, position = 50.0, duration = 100.0, nowMs = 0)
        assertEquals(70.0, twice.value)
        assertEquals(100.0, twice.commitBy(60.0, position = 50.0, duration = 100.0, nowMs = 0).value)
        assertNull(SeekHold().commitBy(10.0, position = 0.0, duration = 0.0, nowMs = 0).value)
    }
}
