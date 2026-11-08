package io.github.mdmrk.cineo.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ImagesTest {
    @Test
    fun aDownsampledImageDoesNotServeAWiderRequest() {
        val small = Decoded("poster", width = 400, full = false)
        assertTrue(small.serves(400))
        assertTrue(small.serves(200))
        assertFalse(small.serves(800))
    }

    @Test
    fun aFullSizeImageServesAnyWidth() {
        assertTrue(Decoded("poster", width = 300, full = true).serves(2000))
    }

    @Test
    fun theCacheIsAQuarterOfTheHeapWithinItsBounds() {
        val mb = 1024L * 1024
        assertEquals(32 * mb, cacheBytes(64 * mb).toLong())
        assertEquals(64 * mb, cacheBytes(256 * mb).toLong())
        assertEquals(128 * mb, cacheBytes(1024 * mb).toLong())
    }
}
