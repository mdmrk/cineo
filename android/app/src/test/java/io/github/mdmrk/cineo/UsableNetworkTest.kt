package io.github.mdmrk.cineo

import org.junit.Assert.assertEquals
import org.junit.Test

class UsableNetworkTest {
    private var usable = 0
    private val network = UsableNetwork { usable++ }

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
    }
}
