package io.github.mdmrk.cineo

import org.junit.Assert.assertEquals
import org.junit.Test

class UsableNetworkTest {
    private var usable = 0
    private var renewals = 0
    private var now = 0L
    private val network = UsableNetwork({ now }) { renew ->
        usable++
        if (renew) renewals++
    }

    @Test
    fun aNetworkAlreadyThereCountsOnceAndRepeatsAreIgnored() {
        network.changed(1, validated = true)
        network.changed(1, validated = true)
        assertEquals(1, usable)
    }

    @Test
    fun aNetworkThatBecomesValidatedCountsAgain() {
        network.changed(1, validated = false)
        network.changed(1, validated = true)
        assertEquals(2, usable)
    }

    @Test
    fun losingValidationDoesNotCount() {
        network.changed(1, validated = true)
        network.changed(1, validated = false)
        assertEquals(1, usable)
    }

    @Test
    fun aNewNetworkCountsAndNoNetworkDoesNot() {
        network.changed(null, validated = false)
        assertEquals(0, usable)
        network.changed(1, validated = true)
        network.changed(2, validated = true)
        assertEquals(2, usable)
    }

    @Test
    fun theSameNetworkCountsAgainAfterItWasLost() {
        network.changed(1, validated = true)
        network.changed(null, validated = false)
        network.changed(1, validated = true)
        assertEquals(2, usable)
        assertEquals(2, renewals)
    }

    @Test
    fun aNetworkThatBecomesValidatedDoesNotRenewAgain() {
        network.changed(1, validated = false)
        network.changed(1, validated = true)
        assertEquals(1, renewals)
    }

    @Test
    fun aShortAbsenceAsksAgainWithoutRenewing() {
        network.changed(1, validated = true)
        network.stopped()
        now += 89_999
        network.changed(1, validated = true)
        assertEquals(2, usable)
        assertEquals(1, renewals)
    }

    @Test
    fun aLongAbsenceRenewsOnce() {
        network.changed(1, validated = true)
        network.stopped()
        now += 90_000
        network.changed(1, validated = false)
        network.changed(1, validated = true)
        assertEquals(3, usable)
        assertEquals(2, renewals)
    }

    @Test
    fun anotherNetworkAfterAShortAbsenceRenews() {
        network.changed(1, validated = true)
        network.stopped()
        now += 1_000
        network.changed(2, validated = true)
        assertEquals(2, usable)
        assertEquals(2, renewals)
    }
}
